Example queries using [employees database](https://github.com/vrajmohan/pgsql-sample-data.git).

Clone and init the database (requires a local PostgreSQL instance):
 
    $ psql -U postgres -c 'CREATE DATABASE employees;'
    $ git clone https://github.com/vrajmohan/pgsql-sample-data.git
    $ psql -U postgres -d employees -f pgsql-sample-data/employee/employees.dump

Execute a PRQL query:

    $ cargo run compile examples/employees/average-title-salary.prql | psql -U postgres -d employees

Also print the query:

    $ cargo run compile my_file.prql | { tee /dev/stderr; echo -e "\nResult:" >&2 } | psql -U postgres -d employees
