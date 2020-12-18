use nix::{sys::stat::Mode, unistd::mkfifo};
use std::collections::HashMap;
use std::env::current_dir;
use std::future::Future;
use std::path::PathBuf;
use tokio::{fs, process, sync::oneshot, task::JoinHandle};
use uuid::Uuid;

struct Task(oneshot::Sender<()>, JoinHandle<()>);

impl Task {
    async fn new(dest: PathBuf, cwd: PathBuf, cmd: Vec<String>) -> anyhow::Result<Task> {
        let dest = if dest.is_absolute() {
            dest
        } else {
            cwd.join(dest)
        };
        anyhow::ensure!(
            !dest.exists(),
            "A file already exists at {}",
            dest.display()
        );
        println!("{} <- `{}`", dest.display(), cmd.iter().map(|s| format!("'{}'", s)).collect::<Vec<_>>().join(" "));
        mkfifo(&dest, Mode::S_IRUSR | Mode::S_IWUSR)?;
        let (tx, mut rx) = oneshot::channel();
        let handle = tokio::spawn(async move {
            let read_task = async {
                let mut writable = fs::OpenOptions::new();
                writable.write(true);
                loop {
                    match writable.open(&dest).await {
                        Err(..) => break,
                        Ok(file) => {
                            println!("Projecting to {}", dest.display());
                            process::Command::new(&cmd[0])
                                .args(&cmd[1..])
                                .current_dir(&cwd)
                                .stdout(file.into_std().await)
                                .spawn()
                                .unwrap()
                                .wait()
                                .await
                                .unwrap();
                            println!("Projection to {} complete", dest.display());
                        }
                    }
                }
            };
            tokio::select! {
                _ = &mut rx => {
                    println!("Removing {}", dest.display());
                }
                _ = read_task => {}
            }
            fs::read_to_string(&dest).await.unwrap();
            fs::remove_file(dest).await.unwrap();
        });
        Ok(Self(tx, handle))
    }

    async fn end(self) -> anyhow::Result<()> {
        self.0.send(()).ok();
        self.1.await?;
        Ok(())
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct Cmd {
    response: PathBuf,
    action: Action,
}

impl Cmd {
    async fn dispatch(action: Action) -> anyhow::Result<i32> {
        let socket_file = crate::socket_file();
        anyhow::ensure!(
            socket_file.exists(),
            "The hologram daemon does not appear to be running."
        );
        let path = std::env::temp_dir().join(Uuid::new_v4().to_string());
        mkfifo(&path, Mode::S_IRUSR | Mode::S_IWUSR)?;
        fs::write(
            socket_file,
            serde_json::ser::to_string(&Cmd {
                response: path.clone(),
                action,
            })
            .unwrap(),
        )
        .await?;
        let response = fs::read_to_string(&path).await?;
        let mut parts = response.splitn(2, ':');
        let code = parts.next().unwrap().parse::<i32>()?;
        if let Some(message) = parts.next() {
            if !message.is_empty() {
                println!("{}", message);
            }
        }
        fs::remove_file(path).await?;
        Ok(code)
    }

    pub async fn add(dest: PathBuf, cmd: Vec<String>) -> anyhow::Result<i32> {
        Self::dispatch(Action::Add {
            dest,
            cwd: current_dir()?,
            cmd,
        })
        .await
    }

    pub async fn remove(dest: PathBuf) -> anyhow::Result<i32> {
        Self::dispatch(Action::Remove { dest }).await
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
enum Action {
    Add {
        dest: PathBuf,
        cwd: PathBuf,
        cmd: Vec<String>,
    },
    Remove {
        dest: PathBuf,
    },
    Quit,
}

#[derive(serde::Serialize, serde::Deserialize)]
enum Response {
    Respond(i32, String),
    Silent,
    Quit,
}

async fn cmd<F, R>(handler: F) -> bool
where
    F: FnOnce(Action) -> R,
    R: Future<Output = anyhow::Result<Response>>,
{
    let cmd = fs::read_to_string(crate::socket_file())
        .await
        .map_err(anyhow::Error::from)
        .and_then(|msg| serde_json::de::from_str(&msg).map_err(Into::into));
    match cmd {
        Ok(Cmd { response, action }) => match handler(action).await {
            Ok(Response::Silent) => {
                fs::write(response, "0:").await.ok();
            }
            Ok(Response::Respond(code, message)) => {
                fs::write(response, format!("{}:{}", code, message))
                    .await
                    .ok();
            }
            Ok(Response::Quit) => {
                fs::write(response, "0:").await.ok();
                return false;
            }
            Err(error) => {
                fs::write(response, format!("1:{}", error)).await.ok();
            }
        },
        Err(..) => return false,
    }
    true
}

#[derive(serde::Serialize, serde::Deserialize)]
struct ConfigTask {
    dest: PathBuf,
    cwd: PathBuf,
    cmd: Vec<String>,
}

#[derive(Default, serde::Serialize, serde::Deserialize)]
struct Config {
    tasks: Vec<ConfigTask>,
}

async fn get_config() -> anyhow::Result<Config> {
    let config_file = crate::config_file();
    if !config_file.exists() {
        Ok(Config::default())
    } else {
        let config = fs::read_to_string(config_file).await?;
        Ok(toml::de::from_str(&config)?)
    }
}

async fn save_config(config: Config) -> anyhow::Result<()> {
    let config_file = crate::config_file();
    if !config_file.exists() {
        fs::create_dir_all(config_file.parent().unwrap()).await?;
    }
    let config = toml::ser::to_string(&config)?;
    fs::write(config_file, config).await?;
    Ok(())
}

pub async fn daemon() -> anyhow::Result<()> {
    let (end, ended) = oneshot::channel();
    let mut end = Some(end);
    let socket_path = crate::socket_file();
    anyhow::ensure!(
        !socket_path.exists(),
        "Hologram daemon socket file {} exists already, is the daemon already running?",
        socket_path.display()
    );
    mkfifo(&socket_path, Mode::S_IRUSR | Mode::S_IWUSR)?;
    ctrlc::set_handler(move || {
        if let Some(end) = end.take() {
            end.send(()).ok();
        }
    })
    .unwrap();
    let mut tasks: HashMap<PathBuf, Task> = HashMap::new();

    let config = get_config().await?;
    for task in config.tasks {
        tasks.insert(task.dest.clone(), Task::new(task.dest, task.cwd, task.cmd).await?);
    }

    let main_task = async {
        loop {
            let cont = cmd(|action| async {
                match action {
                    Action::Add { dest, cwd, cmd } => {
                        let task = Task::new(dest.clone(), cwd.clone(), cmd.clone()).await?;
                        let mut config = get_config().await?;
                        config.tasks.push(ConfigTask { dest: dest.clone(), cwd, cmd });
                        save_config(config).await?;
                        tasks.insert(dest, task);
                        Ok(Response::Silent)
                    }
                    Action::Remove { dest } => match tasks.remove(&dest) {
                        Some(task) => {
                            task.end().await?;
                            let mut config = get_config().await?;
                            config.tasks = config
                                .tasks
                                .into_iter()
                                .filter(|task| task.dest != dest)
                                .collect();
                            save_config(config).await?;
                            Ok(Response::Silent)
                        }
                        None => Ok(Response::Respond(
                            1,
                            format!("No known hologram was found at {}", dest.display()),
                        )),
                    },
                    Action::Quit => Ok(Response::Quit),
                }
            })
            .await;
            if !cont {
                break;
            }
        }
    };

    tokio::select! {
        _ = ended => {
            fs::write(crate::socket_file(), "").await?;
        }
        _ = main_task => {}
    }

    for (_, task) in tasks {
        task.end().await?;
    }

    fs::remove_file(&socket_path).await?;
    Ok(())
}
