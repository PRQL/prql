# Live Editor

<!-- The `container-container` properties already exist on `editor-container`, but we
need to re-specify them since there's another layer (maybe avoidable?). -->

<!-- The sizing is done by having the full height, and then giving directives to fill
the full width excluding the side bar. With just `100%` for the width, the width
attempts to be as large as the full page. I'm not sure why the height is OK at
100% given there is also a header. We also need to give different directives to
Chrome & Firefox! -->

<!-- TODO: remove the "Previous Chapter" bar -->

<style>
    main {
        /*Override the main CSS so that the Editors can be wide*/
        max-width: none !important;
    }
    .container-container {
        position: fixed;
        height: 100%;
        width: -moz-available;
        width: -webkit-fill-available;
        width: stretch;
    }
</style>
<embed class="container-container" type="text/html" src="./dist/index.html">
