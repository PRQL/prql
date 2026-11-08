import "./Sidebar.css";
import { useState } from "react";

function Sidebar({ library, onLoadFile }) {
  function loadFile(section, file) {
    onLoadFile(file, library[section][file]);
  }

  function toggleFolder(id) {
    setOpenFolders((folders) => ({ ...folders, [id]: !folders[id] }));
  }

  function handleClick(section, file, id) {
    if (isFile(file)) {
      loadFile(section, file);
    } else {
      toggleFolder(id);
    }
  }

  function isFile(path) {
    return path.endsWith(".prql");
  }

  const sections = [];
  const [openFolders, setOpenFolders] = useState({});

  for (const [section, files] of Object.entries(library)) {
    const fileRows = [];
    // Whether each row's children should render: the row is itself visible
    // *and* open. `generateBook.cjs` emits parents before their children, so
    // an entry's ancestors are already recorded by the time it's read. Testing
    // only the immediate parent would leave a subtree dangling at the top of
    // the sidebar when a grandparent is collapsed while the parent stays open.
    const childrenVisible = {};
    for (const filename of Object.keys(files)) {
      // The `book` section carries tree metadata past the editor and content;
      // the flat sections (examples, tables, local storage) leave it undefined.
      const [, , depth, parent, id, name] = files[filename];
      const visible =
        parent == null || depth === 0 || Boolean(childrenVisible[parent]);
      childrenVisible[id] = visible && Boolean(openFolders[id]);

      if (!visible) {
        continue;
      }

      fileRows.push(
        <div
          key={filename}
          className={
            "fileRow " +
            (isFile(filename) ? " " : " folderRow ") +
            (openFolders[id] ? " open " : " ")
          }
          style={{ marginLeft: `${12 * (depth ?? 0)}px` }}
          onClick={() => handleClick(section, filename, id)}
        >
          {name ?? filename}
        </div>,
      );
    }

    sections.push(
      <section key={section}>
        <h2>{section}</h2>

        {fileRows}
      </section>,
    );
  }

  return (
    <div className="sidebar">
      <section>
        <h1>PRQL Playground</h1>
      </section>
      <section>
        <h2>External links</h2>
        <div className="fileRow">
          <a
            target="_blank"
            rel="noopener noreferrer"
            href="https://prql-lang.org"
          >
            PRQL Website &#8599;
          </a>
        </div>
        <div className="fileRow">
          <a
            target="_blank"
            rel="noopener noreferrer"
            href="https://prql-lang.org/book/"
          >
            Book &#8599;
          </a>
        </div>
      </section>

      {sections}
    </div>
  );
}

export default Sidebar;
