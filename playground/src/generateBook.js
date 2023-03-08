const { readdir, stat, readFile, writeFile } = require("fs/promises");
const { join, relative, sep, normalize, basename } = require("path");

async function* getAllFiles(dirPath) {
    const files = await readdir(dirPath);

    for (const file of files) {
        const fullPath = join(dirPath, file);
        if ((await stat(fullPath)).isDirectory()) {
            yield* getAllFiles(fullPath);
            yield fullPath;
        } else {
            if (fullPath.endsWith(".prql")) {
                yield fullPath;
            }
        }
    }
}

function depth(path) {
    return path.split(sep).length;
}

function isFile(path) {
    return path.endsWith(".prql");
}

const compName = (a, b) => a.localeCompare(b);
const compDepth = (a, b) => depth(a) - depth(b);
const compFile = (a, b) => {
    if ((a.endsWith(".prql") && b.endsWith(".prql")) ||
        (!a.endsWith(".prql") && !b.endsWith(".prql"))) {
        return 0;
    }
    if (a.endsWith(".prql")) {
        return 1;
    } else {
        return -1;
    }
};

(async () => {
    const fileObject = {};
    const dir = join(__dirname, "..", "..", "book", "tests", "prql");
    const files = [];
    let minDepth = 1e10;
    for await (const file of getAllFiles(dir)) {
        files.push(file);
        minDepth = Math.min(depth(file));
    }
    files.sort(compName);
    files.sort(compDepth);
    files.sort(compFile);
    console.log(files);
    for (const filePath of files) {
        const relativeFile = relative(dir, filePath);
        fileObject[basename(relativeFile)] = [
            "sql",
            isFile(filePath) ? (await readFile(filePath)).toString() : "",
            depth(filePath) - minDepth,
            normalize(join(relativeFile, "..")),
            relativeFile
        ]
    }
    console.log("files", Object.keys(fileObject).length)
    await writeFile(join("src", "book.json"), JSON.stringify(fileObject));
})().catch(console.error);