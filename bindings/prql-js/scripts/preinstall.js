// We want to build the wasm artifacts when we're running locally, but not when
// we're installed as a dependency — then we want to use the included ones. So
// this builds them if we're in `prql-js` only.

// This was written in a few iterations by @max-sixty (who knows no JS) and
// GPT-4, which seems to kinda know JS, but is a bit unsure. So it might not be
// that well implemented; suggestions welcome. I would have thought this would
// be fairly common, so I'm surprised it's not trivial.

const { exec } = require("child_process");
const isDependency = process.env.INIT_CWD !== process.cwd();
console.log(process.env.INIT_CWD);

if (!isDependency) {
  console.log("Installing as root package; building wasm artifacts.");

  exec("npm run build", (error, stdout, stderr) => {
    if (error) {
      console.error(`Error building wasm artifacts: ${error.message}`);
      process.exit(1);
    }

    if (stderr) {
      console.error(`wasm stderr: ${stderr}`);
    }

    console.log(`wasm stdout: ${stdout}`);
  });
} else {
  console.log("Installing as a dependency, using bundled wasm files.");
}
