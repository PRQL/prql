# Live Editor

<!-- The `container-container` properties already exist on `editor-container`, but we
need to re-specify them since there's another layer (maybe avoidable?). -->

<style>
    main {
        /*Override the main CSS so that the Editors can be wide*/
        max-width: none !important;
    }
    .container-container {
        height: 100vh;
        width: 100%;
    }
</style>
<embed class="container-container" type="text/html" src="./dist/index.html">
