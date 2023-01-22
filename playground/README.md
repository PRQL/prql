# PRQL Playground

A fast-feedback compiler from PRQL to SQL, hosted at
<https://prql-lang.org/playground/>

To run locally,
[set up a development environment](https://prql-lang.org/book/contributing/development.html#development-environment),
and then run[^1]:

```sh
task run-playground
```

[^1]: ...which is equivalent to:

    ```sh
    cd playground
    npm install
    npm start
    ```

## Notes

This currently uses `wasm-react-scripts` instead of `react-scripts`, until
<https://github.com/facebook/create-react-app/pull/8303> is merged.
