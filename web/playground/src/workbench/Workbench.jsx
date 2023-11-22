import "./Workbench.css";

import * as prql from "prql-js/dist/bundler";
import React from "react";
import YAML from "yaml";

import Editor, { loader } from "@monaco-editor/react";
import * as monaco from "monaco-editor";
import * as monacoTheme from "./monaco-theme.json";
import prqlSyntax from "./prql-syntax";

import Output from "../output/Output";
import * as duckdb from "./duckdb";

loader.config({ monaco });

class Workbench extends React.Component {
  monaco = null;
  editor = null;

  duckdb = null;

  state = {
    filename: "input.prql",
    prql: "",
    output: null,
    outputTab: "arrow",

    prqlError: null,
    duckdbError: null,
  };

  loadFile(filename, [outputTab, content]) {
    this.setState({ filename, outputTab, prql: content });
    if (this.editor) {
      this.editor.setValue(content);
    }
  }

  componentDidMount() {
    this.props.setCallables({ loadFile: (f, c) => this.loadFile(f, c) });

    if (!this.duckdb) {
      this.duckdb = duckdb.init();
    }
  }

  beforeEditorMount(monaco) {
    this.monaco = monaco;
    monaco.editor.defineTheme("blackboard", monacoTheme);
    monaco.languages.register({ id: "prql", extensions: ["prql"] });
    monaco.languages.setLanguageConfiguration("prql", {
      comments: {
        lineComment: "#",
      },
    });
    monaco.languages.setMonarchTokensProvider("prql", prqlSyntax);
  }

  onEditorMount(editor) {
    this.editor = editor;

    this.compile(editor.getValue());
  }

  async compile(value) {
    this.setState({ prql: value });

    let sql;
    try {
      sql = prql.compile(value);
      this.setState({ prqlError: null });
      this.monaco.editor.setModelMarkers(this.editor.getModel(), "prql", []);
    } catch (e) {
      const errors = JSON.parse(e.message).inner;
      this.setState({ prqlError: errors[0].display });

      const monacoErrors = errors.map((error) => ({
        severity: "error",
        message: error.reason,
        startLineNumber: error.location?.start[0] + 1,
        startColumn: error.location?.start[1] + 1,
        endLineNumber: error.location?.end[0] + 1,
        endColumn: error.location?.end[1] + 1,
      }));
      this.monaco.editor.setModelMarkers(
        this.editor.getModel(),
        "prql",
        monacoErrors
      );
      return;
    }

    let pl;
    try {
      if (sql) {
        pl = prql.prql_to_pl(value);
      }
    } catch (ignored) {}

    let rq;
    try {
      if (pl) {
        rq = prql.pl_to_rq(pl);
      }
    } catch (ignored) {
      console.log(ignored);
    }

    let arrow;
    const c = await (await this.duckdb).connect();
    try {
      arrow = await c.query(sql);
      this.setState({ duckdbError: null });
    } catch (e) {
      this.setState({ duckdbError: e.toString() });
      arrow = null;
    } finally {
      c.close();
    }

    if (pl) {
      pl = YAML.stringify(JSON.parse(pl));
    }
    if (rq) {
      rq = YAML.stringify(JSON.parse(rq));
    }

    const output = { sql, arrow, pl, rq };

    this.setState({ output });
  }

  save() {
    if (!this.editor) return;

    this.props.onSaveFile(this.state.filename, [
      this.state.outputTab,
      this.state.prql,
    ]);
  }

  rename() {
    let filename = prompt(`New name for ${this.state.filename}`);
    if (filename) {
      if (!filename.endsWith(".prql")) {
        filename += ".prql";
      }
      this.setState({ filename });
    }
  }

  render() {
    return (
      <div className="column">
        <div className="tabs">
          <div className="tab">
            <div className="tab-top">
              <div className="tab-title active">{this.state.filename}</div>

              <div className="spacer"></div>

              <button className="action" onClick={() => this.rename()}>
                Rename
              </button>
              <button className="action" onClick={() => this.save()}>
                Save
              </button>
            </div>
            <Editor
              height="10rem"
              defaultLanguage="prql"
              defaultValue={this.state.prql}
              onChange={(v) => this.compile(v)}
              onMount={(e, m) => this.onEditorMount(e, m)}
              beforeMount={(m) => this.beforeEditorMount(m)}
              theme="blackboard"
              options={{
                minimap: { enabled: false },
                scrollBeyondLastLine: false,
                fontSize: 14,
              }}
            />
          </div>

          <Output
            content={this.state.output}
            tab={this.state.outputTab}
            onTabChange={(tab) => this.setState({ outputTab: tab })}
          ></Output>
        </div>

        {/* Display an error message relevant to the tab */}
        {this.state.prqlError && (
          <div className="error-pane">{this.state.prqlError}</div>
        )}
        {this.state.outputTab === "arrow" && this.state.duckdbError && (
          <div className="error-pane">{this.state.duckdbError}</div>
        )}
      </div>
    );
  }
}

export default Workbench;
