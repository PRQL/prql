import "./Sidebar.css";
import React from "react";

function Sidebar({ library, onLoadFile }) {
  function loadFile(section, file) {
    onLoadFile(file, library[section][file]);
  }

  const sections = [];

  for (const [section, files] of Object.entries(library)) {
    const fileRows = [];
    for (const [index, filename] of Object.keys(files).entries()) {
      fileRows.push(
        <div
          key={index}
          className="fileRow"
          onClick={() => loadFile(section, filename)}
        >
          {filename}
        </div>
      );
    }

    sections.push(
      <section key={section}>
        <h2>{section}</h2>

        {fileRows}
      </section>
    );
  }

  return (
    <div className="sidebar">
      <section>
        <h1>PRQL Playground</h1>
      </section>
      <section>
        <h2>External links</h2>
        <a
          className="fileRow"
          target="_blank"
          rel="noopener noreferrer"
          href="https://prql-lang.org"
        >
          PRQL Website &#8599;
        </a>
        <a
          className="fileRow"
          target="_blank"
          rel="noopener noreferrer"
          href="https://prql-lang.org/book/"
        >
          Book &#8599;
        </a>
      </section>

      {sections}
    </div>
  );
}

export default Sidebar;
