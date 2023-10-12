import "./Sidebar.css";
import React from "react";
import { useState } from "react";

function Sidebar({ library, onLoadFile }) {
  function loadFile(section, file) {
    onLoadFile(file, library[section][file]);
  }

  function toggleFolder(id) {
    openFolders[id] = !Boolean(openFolders[id]);
    setOpenFolders(() => ({ ...openFolders }));
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
    for (const [index, filename] of Object.keys(files).entries()) {
      const array = files[filename];
      const depth = array[2];
      const parent = array[3];
      const id = array[4];
      const name = array[5];
      fileRows.push(
        <React.Fragment key={index}>
          {(parent == null || openFolders[parent] || depth === 0) && (
            <div
              className={
                "fileRow " +
                (isFile(filename) ? " " : " folderRow ") +
                (openFolders[id] ? " open " : " ")
              }
              style={{ marginLeft: `${12 * depth}px` }}
              onClick={() => handleClick(section, filename, id)}
            >
              {name ?? filename}
            </div>
          )}
        </React.Fragment>,
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
