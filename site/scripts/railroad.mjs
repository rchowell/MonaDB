/**
 * Renders HTML railroad syntax diagrams from a small sequence DSL.
 *
 * Node kinds:
 *   { t: "keyword" }     terminal (literal token)
 *   { n: "name" }          nonterminal
 *   { opt: [nodes] }       optional group
 *   { or: [branches] }     choice — each branch is a node array
 *   { rep: [nodes] }       zero-or-more repetition
 */

function escapeHtml(text) {
  return String(text)
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;");
}

function renderNodes(nodes) {
  return nodes.map(renderNode).join('<span class="rr-join" aria-hidden="true"></span>');
}

function renderNode(node) {
  if (node.t !== undefined) {
    return `<span class="rr-t">${escapeHtml(node.t)}</span>`;
  }
  if (node.n !== undefined) {
    return `<span class="rr-n">${escapeHtml(node.n)}</span>`;
  }
  if (node.opt) {
    return `<span class="rr-opt"><span class="rr-opt-inner">${renderNodes(node.opt)}</span></span>`;
  }
  if (node.rep) {
    return `<span class="rr-rep"><span class="rr-rep-inner">${renderNodes(node.rep)}</span></span>`;
  }
  if (node.or) {
    const branches = node.or
      .map(
        (branch) =>
          `<span class="rr-branch">${renderNodes(branch)}</span>`,
      )
      .join("");
    return `<span class="rr-or">${branches}</span>`;
  }
  return "";
}

/** Render a complete railroad diagram block. */
export function renderRailroad(nodes) {
  return [
    '<div class="rr">',
    `<div class="rr-track">${renderNodes(nodes)}</div>`,
    "</div>",
  ].join("\n");
}
