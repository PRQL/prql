# CodeMirror with PRQL demo

This is a demo of CodeMirror with PRQL. We don't have any published `lang-prql`
or `prql-lezer` package yet.

## Instructions

Build `prql-lezer` then copy the files from `grammars/prql-lezer/dist/` to
`src/lang-prql/prql-lezer/`. Each package needs `npm install` first, which
fetches the tools its scripts call (`lezer-generator`, `vite`).

```
mkdir src/lang-prql/prql-lezer
cd ../../grammars/prql-lezer/
npm install
npm run build
cp dist/* ../../web/prql-codemirror-demo/src/lang-prql/prql-lezer/
cd ../../web/prql-codemirror-demo/
npm install
npm run dev
```
