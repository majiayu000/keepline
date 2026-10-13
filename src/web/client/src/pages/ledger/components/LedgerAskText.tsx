import { useState } from "react";
import ReactMarkdown, { type Components } from "react-markdown";
import remarkGfm from "remark-gfm";
import styles from "../Workspace.module.css";

const components: Components = {
  a: ({ href, children }) => (
    <a href={href} target="_blank" rel="noopener noreferrer">{children}</a>
  ),
  table: ({ children }) => (
    <div className={styles.markdownTable}><table>{children}</table></div>
  ),
};

export function LedgerAskText({ text }: { text: string }) {
  const [expanded, setExpanded] = useState(false);
  const long = text.length > 600;
  const visible = long && !expanded
    ? `${text.slice(0, 160)}\n\n⋯ 中间内容已折叠 ⋯\n\n${text.slice(-240)}`
    : text;
  return (
    <div>
      <ReactMarkdown className={styles.markdown} remarkPlugins={[remarkGfm]} components={components} skipHtml>
        {visible}
      </ReactMarkdown>
      {long && (
        <button className={styles.actionButton} aria-expanded={expanded} onClick={() => setExpanded(!expanded)}>
          {expanded ? "收起原文" : `展开完整原文（${text.length} 字符）`}
        </button>
      )}
    </div>
  );
}
