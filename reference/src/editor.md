# Live Editor

<!-- The `container-container` properties already exist on `editor-container`, but we
need to re-specify them since there's another layer (maybe avoidable?). -->

<!-- We specify all the hierarchy, from html down, to have height=100%, in order to
get the editors to be full height, and add some reductions in `px` to two
elements, to prevent scroll bars / awkward scrolling when the line breaks run
on. We previously had a different approach, which aligned the content such that
we didn't need scrollbars, but it didn't work well on Chrome. -->

<!-- TODO:
- Should we have a max-width to limit the width for very wide browsers?
- If so, can we get the editors to center?
 -->

<style>
    html, body, .page-wrapper, .page , .container-container {
        height: 100%;
        box-sizing: border-box;
    }
     main {
        height: calc(100% - 150px);
        box-sizing: border-box;
    }
    .content {
        height: calc(100% - 70px);
        box-sizing: border-box;
    }
    main {
        /*Override the main CSS so that the Editors can be wide*/
        max-width: none !important;
    }
    .container-container {
        width: 100%;
    }
    .nav-chapters, .nav-wrapper {
        visibility: hidden;
    }
</style>
<embed class="container-container" type="text/html" src="./dist/index.html">
