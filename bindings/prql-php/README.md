# prql-php

`prql-php` offers PHP bindings to `prql-compiler` crate through FFI.

It provides the `Compiler` class which contains `compile`, `prqlToPL`, `plToRQ`
and `rqToSQL` functions.

It's still at an early stage, and isn't published to Composer. Contributions are
welcome.

## Installation

The [PHP FFI extension](https://www.php.net/manual/en/book.ffi.php) needs to be
enabled. Set `ffi.enable` in your php.ini configuration file to `"true"`.

## Usage

```php
<?php

use Prql\Compiler\Compiler;

$prql = new Compiler();
$result = $prql->compile("from employees");

echo $result->output;
```

## Development

### Environment

A way to establish a dev environment with PHP, the ext-ffi extension and
Composer is to use a [nix flake](https://github.com/loophp/nix-shell). After
installing nix, enable experimental flakes feature:

```
mkdir -p ~/.config/nix
echo "experimental-features = nix-command flakes" >> ~/.config/nix/nix.conf
```

Now you can spawn a shell from `prql-php/`:

```
nix shell github:loophp/nix-shell#env-php81 --impure
```

This will pull-in ext-ffi extension, because it's declared in `composer.json`.

### Building

There is a `build.sh` script that:

- runs cargo to build `libprql_lib`,
- copies `libprql_lib.so` into `lib`,
- copies `libprql_lib.h` into `lib`.

### Tests

```
sh build.sh
./vendor/bin/phpunit tests
```

### Code style

```
./vendor/bin/phpcs --standard=PSR12 src tests
```
