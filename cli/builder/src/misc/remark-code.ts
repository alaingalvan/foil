/**
 * Fork of a few repos:
 * https://github.com/remcohaszing/remark-mdx-code-meta
 * https://github.com/wcoder/highlightjs-line-numbers.js
 *
 * Added support for syntax highlighting with highlight JS
 * As well as line numbers, highlighting lines.
 */

import { Parser } from 'acorn';
import jsx from 'acorn-jsx';
import { BaseNode, Program } from 'estree';
import { Code, Parent, Root } from 'mdast';
import { MdxFlowExpression } from 'mdast-util-mdx';
import { Plugin, Transformer } from 'unified';
import { visit } from 'unist-util-visit';
import hljs from 'highlight.js';
const { getLanguage, highlight } = hljs;
import sanitizeHtml from 'sanitize-html';

const parser = Parser.extend(jsx());

var TABLE_NAME = 'hljs-ln',
    LINE_NAME = 'hljs-ln-line',
    CODE_BLOCK_NAME = 'hljs-ln-code',
    NUMBERS_BLOCK_NAME = 'hljs-ln-numbers',
    NUMBER_LINE_NAME = 'hljs-ln-n',
    DATA_ATTR_NAME = 'data-line-number',
    BREAK_LINE_REGEXP = /\r\n|\r|\n/g;

/**
 * {@link https://wcoder.github.io/notes/string-format-for-string-formating-in-javascript}
 * @param {string} format
 * @param {array} args
 */
function format(format, args) {
    return format.replace(/\{(\d+)\}/g, function (m, n) {
        return args[n] !== undefined ? args[n] : m;
    });
}

function getLines(text) {
    if (text.length === 0) return [];
    return text.split(BREAK_LINE_REGEXP);
}

function addLineNumbersBlockFor(inputHtml, options) {
    var lines = getLines(inputHtml);

    // if last line contains only carriage return remove it
    if (lines[lines.length - 1].trim() === '') {
        lines.pop();
    }

    if (lines.length > 1 || options.singleLine) {
        var html = '';

        for (var i = 0, l = lines.length; i < l; i++) {
            // Close any HTML tags that haven't been closed.
            let line = lines[i].length > 0 ? lines[i] : ' ';
            line = sanitizeHtml(line, {
                allowedClasses: {
                    '*': ['hljs*']
                }
            });
            html += format(
                '<tr>' +
                    '<td class="{0} {1}" {3}="{5}">' +
                    '<div class="{2}" {3}="{5}"></div>' +
                    '</td>' +
                    '<td class="{0} {4}" {3}="{5}">' +
                    '{6}' +
                    '</td>' +
                    '</tr>',
                [
                    LINE_NAME,
                    NUMBERS_BLOCK_NAME,
                    NUMBER_LINE_NAME,
                    DATA_ATTR_NAME,
                    CODE_BLOCK_NAME,
                    i + options.startFrom,
                    line
                ]
            );
        }

        return format('<table class="{0}">{1}</table>', [TABLE_NAME, html]);
    }

    return inputHtml;
}

const transformer: Transformer<Root> = ast => {
    visit(
        ast,
        'code',
        (node: Code, index: number | null, parent: Parent | null) => {
            if (!node.lang) {
                return;
            }

            // 🔣 Escape code like newlines.
            let code = node.value;

            // 🖋️ Process code with highlight.js
            if (node.lang && getLanguage(node.lang)) {
                try {
                    let result = highlight(code, { language: node.lang });
                    if (result.errorRaised) {
                        console.error(result.errorRaised.message);
                    }
                    code = result.value;
                } catch (err) {
                    console.error(err);
                }
            }

            // Add line numbers and escape react prop symbols:
            if (node.meta) {
                code = addLineNumbersBlockFor(code, { startFrom: 1 });
            }
            code = code.replaceAll('{', '&#123;');
            code = code.replaceAll('}', '&#125;');
            code = code.replaceAll('\n', "{'\\n'}");

            const codeProps = `className="language-${node.lang}"`;
            const value = `<div><pre ${node.meta}><code ${codeProps}>${code}</code></pre></div>`;
            const estree = parser.parse(value, {
                ecmaVersion: 'latest'
            }) as BaseNode as Program;
            parent!.children[index!] = {
                type: 'mdxFlowExpression',
                value,
                data: { estree }
            } as MdxFlowExpression;
        }
    );
};

/**
 * A markdown plugin for transforming code metadata.
 *
 * @returns A unified transformer.
 */
const remarkMdxCode: Plugin<[], Root> = () => transformer;

export default remarkMdxCode;
