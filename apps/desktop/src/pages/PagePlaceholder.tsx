import styles from "./PagePlaceholder.module.css";

interface Props {
  title: string;
  /** 设计文档里的章节，例如「6.3」。 */
  section: string;
  /** 原型画板的文件名，例如「Main.dc.html」。 */
  board: string;
  /** BACKLOG 里的条目编号。 */
  backlog: string;
}

/**
 * 还没实现的页面的占位。实现某一页时，把那一页对它的引用删掉；
 * 六页都实现后删掉这个组件。
 */
export function PagePlaceholder({ title, section, board, backlog }: Props) {
  return (
    <section className={styles.placeholder} aria-labelledby="page-title">
      <h1 id="page-title" className={styles.title}>
        {title}
      </h1>
      <p className={styles.note}>这一页还没有实现。</p>
      <dl className={styles.refs}>
        <dt>要求</dt>
        <dd>docs/product-design.md 第 {section} 节</dd>
        <dt>截图</dt>
        <dd>prototype/screens/{board.replace(".dc.html", ".png")}</dd>
        <dt>原型</dt>
        <dd>prototype/project/{board}</dd>
        <dt>待办</dt>
        <dd>{backlog}</dd>
      </dl>
    </section>
  );
}
