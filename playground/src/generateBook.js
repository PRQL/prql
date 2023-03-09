const { readdir, stat, readFile, writeFile } = require("fs/promises");
const { join, relative, sep, normalize, basename } = require("path");

async function* getAllFiles(dirPath) {
    const files = await readdir(dirPath);
    files.sort((a, b) => (+isFile(a)) - (+isFile(b)))

    for (const file of files) {
        const fullPath = join(dirPath, file);
        if ((await stat(fullPath)).isDirectory()) {
            yield fullPath;
            yield* getAllFiles(fullPath);
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

(async () => {
    const fileObject = {};
    const dir = join(__dirname, "..", "..", "book", "tests", "prql");
    const files = [];
    let minDepth = 1e10;
    for await (const file of getAllFiles(dir)) {
        files.push(file);
        minDepth = Math.min(depth(file));
    }
    for (const filePath of files) {
        const relativeFile = relative(dir, filePath);
        fileObject[(relativeFile)] = [
            "sql", // editor
            isFile(filePath) ? (await readFile(filePath)).toString() : "", // content
            depth(filePath) - minDepth, // depth
            normalize(join(relativeFile, "..")), // parent
            relativeFile, // id
            basename(relativeFile), // name
        ]
    }
    await writeFile(join("src", "book.json"), JSON.stringify(fileObject));
})().catch(e => {
    console.error(e);
    process.exit(1);
});