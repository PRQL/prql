import "./Output.css";

import React from "react";

import { Light as SyntaxHighlighter } from "react-syntax-highlighter";
import sql from "react-syntax-highlighter/dist/esm/languages/hljs/sql";
import yaml from "react-syntax-highlighter/dist/esm/languages/hljs/yaml";

SyntaxHighlighter.registerLanguage("sql", sql);
SyntaxHighlighter.registerLanguage("yaml", yaml);

function Tab(props) {
  return (
    <button
      className={`tab-title ${props.parent.tab === props.name ? "active" : ""}`}
      onClick={() => props.parent.onTabChange(props.name)}
    >
      {props.text}
    </button>
  );
}

class Output extends React.Component {
  state = {
    justCopied: false,
  };

  render() {
    return (
      <div className="tab">
        <div className="tab-top">
          <Tab text="Compiled&nbsp;SQL" name="sql" parent={this.props} />
          <Tab text="Query&nbsp;Results" name="arrow" parent={this.props} />
          <Tab text="PL.yaml" name="pl" parent={this.props} />
          <Tab text="RQ.yaml" name="rq" parent={this.props} />

          <div className="spacer"></div>

          <button className="action" onClick={() => this.copyOutput()}>
            {this.state.justCopied ? "Copied!" : "Copy to clipboard"}
          </button>
        </div>

        {this.renderContent()}
      </div>
    );
  }

  renderContent() {
    if (!this.props.content) {
      return <div className="tab-content"></div>;
    }
    if (this.props.tab === "sql") {
      return (
        <SyntaxHighlighter language="sql" useInlineStyles={false}>
          {this.props.content.sql}
        </SyntaxHighlighter>
      );
    }
    if (this.props.tab === "arrow" && this.props.content.arrow) {
      const arrow = this.props.content.arrow;

      const header = arrow.schema.fields.map((f, index) => {
        return <th key={index}>{f.name}</th>;
      });

      const converters = arrow.schema.fields.map((f) => {
        const typ = f.type.toString();
        if (typ.startsWith("Timestamp")) {
          // TODO: handle timezone (which Date does not support)

          // HACK: due to bug in arrow or duckdb, we are getting MICROSECOND here,
          // but the values are actually milliseconds. I'm not sure what is going on,
          // so let's just assume the values will always be in milliseconds.
          /*
          if (typ.endsWith("<SECOND>")) {
            return (x) => new Date(x * 1000).toISOString();
          }
          if (typ.endsWith("<MILLISECOND>")) {
            return (x) => new Date(x).toISOString();
          }
          if (typ.endsWith("<MICROSECOND>")) {
            return (x) => new Date(x / 1000).toISOString();
          }
          if (typ.endsWith("<NANOSECOND>")) {
            return (x) => new Date(x / 1000000).toISOString();
          }
          */
          return (x) => new Date(x).toISOString();
        }
        return (x) => x;
      });

      const data = arrow.toArray().map((x) => [...x]);
      const rows = data.map((x, index) => {
        const cells = x.map(([_name, value], index) => (
          <td key={index}>{"" + converters[index](value)}</td>
        ));

        return <tr key={index}>{cells}</tr>;
      });

      // console.log(arrow, arrow.schema.fields, arrow.toArray());

      return (
        <div className="tab-content arrow">
          <table className="tab-content">
            <thead>
              <tr>{header}</tr>
            </thead>
            <tbody>{rows}</tbody>
          </table>
        </div>
      );
    }
    if (this.props.tab === "pl" && this.props.content.pl) {
      return (
        <SyntaxHighlighter language="yaml" useInlineStyles={false}>
          {this.props.content.pl}
        </SyntaxHighlighter>
      );
    }
    if (this.props.tab === "rq" && this.props.content.rq) {
      return (
        <SyntaxHighlighter language="yaml" useInlineStyles={false}>
          {this.props.content.rq}
        </SyntaxHighlighter>
      );
    }
    return <div className="tab-content"></div>;
  }

  async copyOutput() {
    try {
      await navigator.clipboard.writeText(this.props.content[this.props.tab]);

      this.setState({ justCopied: true });

      await new Promise((r) => window.setTimeout(r, 2000));
      this.setState({ justCopied: false });
    } catch (e) {
      console.error(e);
    }
  }
}

export default Output;
