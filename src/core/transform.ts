/**
 * AST instrumentation: provenance tracking and call hooks.
 *
 * After the text-level downgrade, every root is a plain const wrapped in
 * __runes.markRoot(...). This pass walks the TypeScript AST and:
 *
 * 1. Tracks provenance conservatively within each scope: a binding whose
 *    initializer references a tracked name inherits its root set, and so
 *    do later assignments. Function declarations that reference tracked
 *    names from an outer scope are tracked as closures.
 * 2. Wraps any call expression that references tracked names (callee or
 *    arguments) in __runes.call(label, roots, () => original, ...). The
 *    thunk preserves 'this' for method calls and lets the runtime record
 *    enter/exit/duration with unwind safety.
 * 3. Wraps derived bindings in __runes.derive(...) to emit value_derive
 *    events.
 * 4. Injects the runtime import.
 */
import ts from "ts5";
import { downgradeRunes } from "./syntax.ts";

/** Options controlling instrumentation of one source file. */
export interface InstrumentOptions {
  /** Display path recorded in trace events. */
  fileName: string;
  /** Import specifier for the generated runtime module. */
  runtimeSpecifier: string;
  /** Root declaration keyword. Defaults to 'rune'. */
  keyword?: string;
}

const RUNTIME_NS = "__runes";
const MAX_LABEL_LENGTH = 80;

/** Transpiles TypeScript source to plain ESM JavaScript. */
export function transpileToJs(source: string, fileName: string): string {
  return ts.transpileModule(source, {
    fileName,
    compilerOptions: {
      module: ts.ModuleKind.ESNext,
      target: ts.ScriptTarget.ES2022,
    },
  }).outputText;
}

type RootSet = Set<string>;

interface Scope {
  locals: Map<string, RootSet>;
}

/** Instruments one TypeScript source file and returns printed output. */
export function instrumentSource(source: string, options: InstrumentOptions): string {
  const downgraded = downgradeRunes(source, options.fileName, options.keyword ?? "rune");
  const sourceFile = ts.createSourceFile(
    options.fileName,
    downgraded.source,
    ts.ScriptTarget.ESNext,
    true,
    ts.ScriptKind.TS,
  );
  const factory = ts.factory;
  const scopes: Scope[] = [{ locals: new Map() }];

  function declare(name: string, roots: RootSet): void {
    scopes[scopes.length - 1].locals.set(name, roots);
  }

  function lookup(name: string): RootSet | undefined {
    for (let i = scopes.length - 1; i >= 0; i--) {
      const scope = scopes[i].locals;
      if (scope.has(name)) return scope.get(name);
    }
    return undefined;
  }

  /** Unions the root sets of every tracked identifier under node. */
  function referencedRoots(node: ts.Node): RootSet {
    const roots: RootSet = new Set();
    const walk = (current: ts.Node): void => {
      if (ts.isIdentifier(current)) {
        const found = lookup(current.text);
        if (found) for (const root of found) roots.add(root);
      }
      ts.forEachChild(current, walk);
    };
    walk(node);
    return roots;
  }

  function positionOf(node: ts.Node): { line: number; column: number } {
    const pos = sourceFile.getLineAndCharacterOfPosition(node.getStart(sourceFile));
    return { line: pos.line + 1, column: pos.character + 1 };
  }

  function runtimeCall(method: string, args: readonly ts.Expression[]): ts.CallExpression {
    return factory.createCallExpression(
      factory.createPropertyAccessExpression(factory.createIdentifier(RUNTIME_NS), method),
      undefined,
      args,
    );
  }

  function positionArgs(node: ts.Node): ts.Expression[] {
    const pos = positionOf(node);
    return [
      factory.createStringLiteral(options.fileName),
      factory.createNumericLiteral(pos.line),
      factory.createNumericLiteral(pos.column),
    ];
  }

  function rootsLiteral(roots: RootSet): ts.Expression {
    return factory.createArrayLiteralExpression(
      [...roots].sort().map((root) => factory.createStringLiteral(root)),
    );
  }

  function isRuntimeCall(node: ts.CallExpression): boolean {
    return (
      ts.isPropertyAccessExpression(node.expression) &&
      ts.isIdentifier(node.expression.expression) &&
      node.expression.expression.text === RUNTIME_NS
    );
  }

  function isFunctionLike(
    node: ts.Node,
  ): node is
    | ts.FunctionDeclaration
    | ts.FunctionExpression
    | ts.ArrowFunction
    | ts.MethodDeclaration
    | ts.ConstructorDeclaration
    | ts.GetAccessorDeclaration
    | ts.SetAccessorDeclaration {
    return (
      ts.isFunctionDeclaration(node) ||
      ts.isFunctionExpression(node) ||
      ts.isArrowFunction(node) ||
      ts.isMethodDeclaration(node) ||
      ts.isConstructorDeclaration(node) ||
      ts.isGetAccessorDeclaration(node) ||
      ts.isSetAccessorDeclaration(node)
    );
  }

  function pushFunctionScope(node: ts.FunctionLikeDeclaration): void {
    const scope: Scope = { locals: new Map() };
    scopes.push(scope);
    for (const param of node.parameters) {
      if (ts.isIdentifier(param.name)) {
        scope.locals.set(param.name.text, new Set());
      }
    }
  }

  const visitor: ts.Visitor = (node) => {
    if (isFunctionLike(node)) {
      pushFunctionScope(node);
      // A function that closes over tracked names is itself tracked, so
      // later calls to it carry the captured roots.
      const captured =
        ts.isFunctionDeclaration(node) && node.body ? referencedRoots(node.body) : undefined;
      const visited = ts.visitEachChild(node, visitor, context);
      scopes.pop();
      if (captured && node.name && ts.isIdentifier(node.name)) {
        declare(node.name.text, captured);
      }
      return visited;
    }

    if (ts.isBlock(node) || ts.isCaseClause(node)) {
      scopes.push({ locals: new Map() });
      const visited = ts.visitEachChild(node, visitor, context);
      scopes.pop();
      return visited;
    }

    if (ts.isVariableDeclaration(node) && node.initializer && ts.isIdentifier(node.name)) {
      // A markRoot call produced by the downgrade: the binding is a root
      // carrying its own name, and its initializer stays uninstrumented.
      if (ts.isCallExpression(node.initializer) && isRuntimeCall(node.initializer)) {
        declare(node.name.text, new Set([node.name.text]));
        return node;
      }
      const roots = referencedRoots(node.initializer);
      const initializer =
        ts.visitNode(node.initializer, visitor, ts.isExpression) ?? node.initializer;
      declare(node.name.text, roots);
      if (roots.size === 0) {
        return factory.updateVariableDeclaration(
          node,
          node.name,
          node.exclamationToken,
          node.type,
          initializer,
        );
      }
      const derived = runtimeCall("derive", [
        factory.createStringLiteral(node.name.text),
        initializer,
        rootsLiteral(roots),
        ...positionArgs(node),
      ]);
      return factory.updateVariableDeclaration(
        node,
        node.name,
        node.exclamationToken,
        node.type,
        derived,
      );
    }

    if (
      ts.isBinaryExpression(node) &&
      node.operatorToken.kind === ts.SyntaxKind.EqualsToken &&
      ts.isIdentifier(node.left)
    ) {
      const right = ts.visitNode(node.right, visitor, ts.isExpression) ?? node.right;
      const roots = referencedRoots(right);
      declare(node.left.text, roots);
      const wrapped =
        roots.size > 0
          ? runtimeCall("derive", [
              factory.createStringLiteral(node.left.text),
              right,
              rootsLiteral(roots),
              ...positionArgs(node),
            ])
          : right;
      return factory.updateBinaryExpression(node, node.left, node.operatorToken, wrapped);
    }

    if (ts.isCallExpression(node)) {
      if (isRuntimeCall(node)) return node;
      if (
        node.expression.kind === ts.SyntaxKind.SuperKeyword ||
        node.expression.kind === ts.SyntaxKind.ImportKeyword
      ) {
        return ts.visitEachChild(node, visitor, context);
      }
      const visited = ts.visitEachChild(node, visitor, context);
      const roots = referencedRoots(visited);
      if (roots.size === 0) return visited;
      const label = node.expression.getText(sourceFile).slice(0, MAX_LABEL_LENGTH);
      const thunk = factory.createArrowFunction(
        undefined,
        undefined,
        [],
        undefined,
        factory.createToken(ts.SyntaxKind.EqualsGreaterThanToken),
        visited,
      );
      return runtimeCall("call", [
        factory.createStringLiteral(label),
        rootsLiteral(roots),
        thunk,
        ...positionArgs(node),
      ]);
    }

    return ts.visitEachChild(node, visitor, context);
  };

  let context!: ts.TransformationContext;
  let transformed!: ts.SourceFile;
  ts.transform(sourceFile, [
    (ctx) => {
      context = ctx;
      return (sf) => {
        // The visitor never removes the root, so the fallback is moot.
        transformed = ts.visitNode(sf, visitor, ts.isSourceFile) ?? sf;
        return transformed;
      };
    },
  ]);
  const importDecl = factory.createImportDeclaration(
    undefined,
    factory.createImportClause(
      false,
      undefined,
      factory.createNamespaceImport(factory.createIdentifier(RUNTIME_NS)),
    ),
    factory.createStringLiteral(options.runtimeSpecifier),
  );
  const updated = factory.updateSourceFile(transformed, [importDecl, ...transformed.statements]);
  return ts.createPrinter().printFile(updated);
}
