# enf

Slightly nicer than `/usr/bin/env`: `enf` runs a program in a modified
environment, and can read the modifications from dotenv files.

## Install

```sh
cargo install enf
```

## Usage

```
enf [OPTIONS] [KEY=VALUE]... [PROGRAM [ARG]...]

Options:
  -f, --file <FILE>  Set variables from a dotenv file; may be repeated
  -h, --help         Print this help
```

Options must precede any `KEY=VALUE` settings. Later files override earlier
ones, and `KEY=VALUE` settings override files.

```sh
enf -f .env -f .env.local PORT=8080 cargo run
```

Without a program, `enf` runs `/usr/bin/env`, which prints the resulting
environment.

## Exit status

| Status | Meaning                                         |
| ------ | ----------------------------------------------- |
| 1      | A dotenv file could not be opened               |
| 2      | A file option has no path                       |
| 3      | A dotenv file could not be read or parsed       |
| 126    | The program was found but could not be executed |
| 127    | The program was not found                       |

Otherwise, `enf` replaces itself with the program, so the exit status is the
program's.

## License

[MIT](LICENSE)
