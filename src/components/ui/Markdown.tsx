import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";

interface MarkdownProps {
  text: string;
}

/** 拆解产出 md 的摩卡主题渲染(GFM 表格必须:节拍表/钩子链/节奏数据)。 */
export function Markdown({ text }: MarkdownProps) {
  return (
    <div className="max-w-3xl text-[13px] leading-relaxed text-mocha-subtext">
      <ReactMarkdown
        remarkPlugins={[remarkGfm]}
        components={{
          h1: ({ children }) => (
            <h1 className="mb-3 mt-2 text-lg font-semibold text-mocha-text">{children}</h1>
          ),
          h2: ({ children }) => (
            <h2 className="mb-2 mt-5 border-b border-mocha-rim/40 pb-1 text-base font-semibold text-mocha-text">
              {children}
            </h2>
          ),
          h3: ({ children }) => (
            <h3 className="mb-1.5 mt-4 text-sm font-semibold text-mocha-text">{children}</h3>
          ),
          h4: ({ children }) => (
            <h4 className="mb-1 mt-3 text-[13px] font-semibold text-mocha-text">{children}</h4>
          ),
          p: ({ children }) => <p className="my-1.5">{children}</p>,
          ul: ({ children }) => <ul className="my-2 list-disc space-y-1 pl-5">{children}</ul>,
          ol: ({ children }) => <ol className="my-2 list-decimal space-y-1 pl-6">{children}</ol>,
          strong: ({ children }) => (
            <strong className="font-semibold text-mocha-text">{children}</strong>
          ),
          blockquote: ({ children }) => (
            <blockquote className="my-2 border-l-2 border-mocha-accent/50 pl-3 text-mocha-muted">
              {children}
            </blockquote>
          ),
          code: ({ children }) => (
            <code className="rounded bg-mocha-crust px-1 py-0.5 font-mono text-[11.5px]">
              {children}
            </code>
          ),
          hr: () => <hr className="my-4 border-mocha-rim/40" />,
          a: ({ children, href }) => (
            <a href={href} className="text-mocha-blue underline">
              {children}
            </a>
          ),
          // 宽表(节拍表 7 列)在自身容器内横向滚动,不撑破页面。
          table: ({ children }) => (
            <div className="my-3 overflow-x-auto">
              <table className="w-full border-collapse text-xs">{children}</table>
            </div>
          ),
          th: ({ children }) => (
            <th className="border-b border-mocha-rim px-2 py-1.5 text-left font-semibold text-mocha-text">
              {children}
            </th>
          ),
          td: ({ children }) => (
            <td className="border-b border-mocha-rim/30 px-2 py-1 align-top">{children}</td>
          ),
        }}
      >
        {text}
      </ReactMarkdown>
    </div>
  );
}
