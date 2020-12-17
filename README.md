# Hologram

Create holographic files, projecting the output of an arbitrary command.

Use at your own risk. This is probably a sketchy thing to do. I don't know lots
about system security.

## Usage

```sh
holo daemon start # Start the daemon, which will begin projecting the files
holo add file.txt -- ./file.sh # Start projecting the output of ./file.sh into file.txt
holo daemon stop # Stop the daemon, removing any projected files.
```

## Configuration

Previously created holograms are recorded in the configuration file, allowing
the daemon to immediately resume where it left off.

The configuration is in TOML syntax, and may be modified manually. Note that,
when modified, it will not pick up changes until the daemon is restarted:

```toml
[[hologram]]
destination = "/home/<user>/file.txt"  # the file to project
cwd = "/home/<user>/" # the CWD to run the command from
cmd = "./file.sh" # The command to run, often a script located relative to cwd
```
