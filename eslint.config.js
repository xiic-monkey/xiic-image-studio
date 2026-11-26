// 静态防线：只开 react-hooks 两条规则，专门拦「早退 return 之后还有 hook」
// 这类会导致 React 白屏的错误。不引入 recommended 全套，避免带出一堆
// 与本问题无关的 any / 未用变量报错，冲淡这条防线的信号。
import tseslint from "typescript-eslint";
import reactHooks from "eslint-plugin-react-hooks";

export default [
  {
    files: ["src/**/*.{ts,tsx}"],
    languageOptions: {
      // eslint 9 flat config 里 tseslint.config() 不自动挂 parser，必须显式指定
      parser: tseslint.parser,
      parserOptions: {
        ecmaVersion: "latest",
        sourceType: "module",
        ecmaFeatures: { jsx: true },
      },
    },
    plugins: { "react-hooks": reactHooks },
    rules: {
      "react-hooks/rules-of-hooks": "error",
      "react-hooks/exhaustive-deps": "warn",
    },
  },
];
