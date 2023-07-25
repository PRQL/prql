const { readdir, stat, readFile, writeFile } = require("fs/promises");
const { join, relative, sep, normalize, basename } = require("path");
const { EOL } = require("os");

/**
 * Get all markdown files in given dir
 * @param {string} dirPath
 */
async function* getAllFiles(dirPath) {
  const files = await readdir(dirPath);
  files.sort((a, b) => +isFile(a) - +isFile(b));

  for (const file of files) {
    const fullPath = join(dirPath, file);
    if ((await stat(fullPath)).isDirectory()) {
      yield fullPath;
      yield* getAllFiles(fullPath);
    } else {
      if (fullPath.endsWith(".md")) {
        yield fullPath;
      }
    }
  }
}

function depth(path) {
  return path.split(sep).length;
}

function isFile(path) {
  return path.endsWith(".md");
}

/**
 * Get all prql code snippets from a markdown file
 * @param {string} content
 * @param {string} file
 * @returns {{title:string, prql:string}[]}
 */
function getSnippets(content, file) {
  const name = file.trim().toLowerCase().replace(/\s/g, "_");
  let heading = "";
  let prql = null;
  const arr = [];
  let index = 1;
  const titles = new Set();
  content.split(/\r\n|\n/).forEach((line) => {
    if (prql == null && line.startsWith("#") && line.includes("# ")) {
      const spaceIndex = line.indexOf("# ");
      heading = line
        .slice(spaceIndex + 2)
        .trim()
        .toLowerCase()
        .replace(/\s/g, "_");
      return;
    }
    if (line.trim() === "```prql") {
      prql = "";
      return;
    }
    if (prql != null && line.trim() === "```") {
      let title = heading || name;
      if (titles.has(title)) {
        title += `_${++index}`;
      } else {
        index = 1;
      }
      arr.push({
        title: title + ".prql",
        prql: prql.trim(),
      });
      titles.add(title);
      prql = null;
      return;
    }
    if (prql != null) {
      prql = prql + line + EOL;
    }
  });
  if (arr.length !== titles.size) {
    throw new Error("duplicate titles");
  }
  return arr;
}

(async () => {
  const fileObject = {};
  const dir = join(__dirname, "..", "book", "src");
  const files = [];
  let minDepth = 1e10;
  for await (const file of getAllFiles(dir)) {
    files.push(file);
    minDepth = Math.min(depth(file));
  }

  for (const filePath of files) {
    const relativeFile = relative(dir, filePath);
    const snippets = isFile(filePath)
      ? getSnippets(
          (await readFile(filePath)).toString(),
          basename(filePath).replace(/\..+/g, "").trim(),
        )
      : [];
    if (!snippets.length && isFile(filePath)) {
      continue;
    }
    const dept = depth(filePath) - minDepth;
    fileObject[relativeFile] = [
      "", // editor
      "", // content
      dept, // depth
      normalize(join(relativeFile, "..")), // parent
      relativeFile, // id
      basename(relativeFile).replace(/\..+/g, "").trim(), // name
    ];
    for (const snippet of snippets) {
      const id = relativeFile + "_" + snippet.title;
      fileObject[id] = [
        "sql",
        snippet.prql,
        dept + 1,
        relativeFile,
        id,
        snippet.title,
      ];
    }
  }

  // remove empty folders
  const keys = Object.keys(fileObject);
  const parents = new Set();
  Object.values(fileObject).forEach((v) => parents.add(v[3]));
  for (const key of keys) {
    if (!parents.has(key) && !fileObject[key][0]) {
      delete fileObject[key];
    }
  }

  await writeFile(join("src", "book.json"), JSON.stringify(fileObject));
})().catch((e) => {
  console.error(e);
  process.exit(1);
});
