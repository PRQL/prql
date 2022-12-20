import "./Workbench.css";

import React from "react";
import * as prql from "prql-js/dist/bundler";

import * as monacoTheme from "./monaco-theme.json";
import * as monaco from "monaco-editor";
import Editor, { loader } from "@monaco-editor/react";
import prqlSyntax from "./prql-syntax";

import { Light as SyntaxHighlighter } from "react-syntax-highlighter";
import sql from "react-syntax-highlighter/dist/esm/languages/hljs/sql";
import Output from "../output/Output";
import * as duckdb from "./duckdb";

SyntaxHighlighter.registerLanguage("sql", sql);

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

    this.duckdb = duckdb.init();
  }

  beforeEditorMount(monaco) {
    this.monaco = monaco;
    monaco.editor.defineTheme("blackboard", monacoTheme);
    monaco.languages.register({ id: "prql", extensions: ["prql"] });
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
      const error = JSON.parse(e.message).inner[0];
      this.setState({ prqlError: error.display });

      const errors = [
        {
          severity: "error",
          message: error.reason,
          startLineNumber: error.location?.start_line + 1,
          startColumn: error.location?.start_column + 1,
          endLineNumber: error.location?.end_line + 1,
          endColumn: error.location?.end_column + 1,
        },
      ];
      this.monaco.editor.setModelMarkers(
        this.editor.getModel(),
        "prql",
        errors
      );
      return;
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

    const output = { sql, arrow };

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
              }}
            />
          </div>

          <Output
            content={this.state.output}
            tab={this.state.outputTab}
            onTabChange={(tab) => this.setState({ outputTab: tab })}
          ></Output>
        </div>

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
