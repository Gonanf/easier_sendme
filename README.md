
# Easier Readme

A simpler version of Sendme for learning




## Usage/Examples

```bash
A simpler version of Sendme for learning

Usage: easier_sendme <COMMAND>

Commands:
  send     
  receive  
  help     Print this message or the help of the given subcommand(s)

Options:
  -h, --help     Print help
  -V, --version  Print version
```

Example of the sending side:
```bash
easier_sendme send -p <PATH> -d <DATABASE>
```
Set <PATH> to the directory of file to send, and a <DATABASE> path to save and load the files/hashes.

If both are empty, it will open a temporal default.

Example of the receiver side:
```bash
easier_sendme receive <TICKET> -p <PATH> 
```
Get the <TICKET> from the sender and optionaly set a <PATH> to save those files.

You will not be able to set a database since a receiver only needs to get the data that is asking for.


## Run Locally

Clone the project

```bash
  git clone https://github.com/Gonanf/easier_sendme
```

Go to the project directory

```bash
  cd my-project
```

Build the program

```bash
  cargo build
```


