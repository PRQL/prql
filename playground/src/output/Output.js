import "./Output.css";

import React from "react";

import { Light as SyntaxHighlighter } from "react-syntax-highlighter";

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
          <Tab text="output.sql" name="sql" parent={this.props} />
          <Tab text="output.arrow" name="arrow" parent={this.props} />

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

      const data = arrow.toArray().map((x) => [...x]);
      const rows = data.map((x, index) => {
        const cells = x.map(([_name, value], index) => (
          <td key={index}>{"" + value}</td>
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
    return <div className="tab-content"></div>;
  }

  async copyOutput() {
    try {
      await navigator.clipboard.writeText(this.state.sql);

      this.setState({ justCopied: true });

      await new Promise((r) => window.setTimeout(r, 2000));
      this.setState({ justCopied: false });
    } catch (e) {
      console.error(e);
    }
  }
}

export default Output;
