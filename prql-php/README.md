# prql-php
`prql-php` offers PHP bindings through FFI.

It provides the `Compiler` class which contains the `toJson` and `toSql` methods.

It's still at an early stage, and isn't published to Composer. Contributions are welcome.

## Installation
The [PHP FFI extension](https://www.php.net/manual/en/book.ffi.php) needs to be enabled.
Set `ffi.enable` in your php.ini configuration file to `"true"`.

## Usage
```php
<?php

use Prql\Compiler\Compiler;

$prql = new Compiler();
$result = $prql->toSql("from employees");
```
