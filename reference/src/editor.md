# Live Editor

<!-- The `container-container` properties already exist on `editor-container`, but we
need to re-specify them since there's another layer (maybe avoidable?). -->

<!-- We specify all the hierarchy, from html down, to have height=100%, in order to
get the editors to be full height.
We then need to add `overflow: hidden` to hide the scrollbars — we previously
had a different approach, which aligned the content such that we didn't need
scrollbars, but it didn't work well on Chrome. -->

<!-- TODO:
- Should we have a max-width to limit the width for very wide browsers?
- If so, can we get the editors to center?
 -->

<style>
    html, body, .page-wrapper, .page, .content, main, .container-container {
        height: 100%;
        overflow: hidden;
    }
    main {
        /*Override the main CSS so that the Editors can be wide*/
        max-width: none !important;
    }
    .container-container {
        width: 100%;
    }
    .nav-chapters {
        visibility: hidden;
    }
</style>
<embed class="container-container" type="text/html" src="./dist/index.html">
