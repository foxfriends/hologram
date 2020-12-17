# Hologram

Create holographic files, projecting the output of an arbitrary command into a file.

## Warnings

__Use at your own risk__. This is probably a sketchy thing to do. I don't know lots
about system security.

Each time the file is read, the hologram daemon will run the provided command, and
pipe its output (from standard output) to the holographic file. Since this will be run
*on every read* it is __*strongly recommended*__ that this script does not do anything
other than read other files and print things.

The hologram daemon will run the provided command using whatever user you have configured
it to run as. It is __*strongly recommended*__ to ensure that this user is properly
permissioned (with very low/read only permissions or something) so as not to allow
anyone to replace a script that is being run and do something terrible or read sensitive
information.

## Usage

```sh
hologram start # Start the daemon, which will begin projecting the files
hologram add file.txt -- ./file.sh # Start projecting the output of ./file.sh into file.txt
hologram remove file.txt # Stop projecting to file.txt
```

It is not recommended to delete the holographic files while the program is running. I
haven't tested it, and don't know what it will do. Let's call it undefined behaviour.

The daemon is really intended to be run in `systemd`. I don't know how to do that yet
but will provide information on that subject soon.

## Configuration

> Not yet implemented.

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
