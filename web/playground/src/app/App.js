import "./App.css";
import Workbench from "../workbench/Workbench";
import Sidebar from "../sidebar/Sidebar";
import examples from "../examples";
import book from "../book.json";
import * as duckdb from "../workbench/duckdb";

import React from "react";

function loadLocalStorage() {
  return JSON.parse(localStorage.getItem("files")) || {};
}

function saveLocalStorage(files) {
  return localStorage.setItem("files", JSON.stringify(files));
}

const chinook = duckdb.CHINOOK_TABLES.reduce((lib, table) => {
  return Object.assign(lib, {
    [table + ".prql"]: ["arrow", `from ${table}\ntake 10`],
  });
}, {});

class App extends React.Component {
  workbenchActions = null;
  state = {
    library: {
      examples,
      tables: chinook,
      book,
      "local storage": loadLocalStorage(),
    },
  };

  setWorkbenchActions = (callables) => {
    this.workbenchActions = callables;
  };

  componentDidMount() {
    let defaultFile = "introduction.prql";
    this.workbenchActions.loadFile(defaultFile, examples[defaultFile]);
  }

  saveFile(filename, content) {
    const localStorage = {
      ...this.state.library["local storage"],
      [filename]: content,
    };
    this.setState({
      library: { ...this.state.library, "local storage": localStorage },
    });
    saveLocalStorage(localStorage);
  }

  render() {
    return (
      <div className="main">
        <Sidebar
          library={this.state.library}
          onLoadFile={(f, c) => this.workbenchActions.loadFile(f, c)}
        />

        <Workbench
          setCallables={this.setWorkbenchActions}
          onSaveFile={(f, c) => this.saveFile(f, c)}
        />
      </div>
    );
  }
}

export default App;
