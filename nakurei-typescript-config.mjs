import { defineConfig } from "eslint/config";

export default defineConfig([
  {
    rules: {
      /*
       * 以下はjs.configs.recommended（eslint:recommended）にないルール
       * かつ、tseslint.configs.strictTypeCheckedおよび
       * tseslint.configs.stylisticTypeCheckedに被らないルール
       */

      // 配列のメソッドのコールバック関数でreturn文を強制
      "array-callback-return": "error",
      // ループ内でのawaitを禁止（Promise.allを使おう）
      "no-await-in-loop": "error",
      // コンストラクタ内でのreturn文を禁止
      "no-constructor-return": "error",
      // 重複importをエラーとして報告
      "no-duplicate-imports": "error",
      // ネスト内での関数定義/宣言を禁止
      "no-inner-declarations": "error",
      // Promiseのexecutorでのreturn文を禁止
      "no-promise-executor-return": "error",
      // 自分自身との比較をエラーとして報告
      "no-self-compare": "error",
      // 文字列内のテンプレートリテラルをエラーとして報告
      "no-template-curly-in-string": "error",
      // ループ内での条件変更を禁止
      "no-unmodified-loop-condition": "error",
      // 1回しかループしないループをエラーとして報告
      "no-unreachable-loop": "error",
      // 定義前の変数の使用を禁止
      "no-use-before-define": "error",
      // 変数への不要な代入を禁止
      "no-useless-assignment": "error",
      // awaitやyieldによる競合状態を引き起こしうる代入を禁止
      "require-atomic-updates": "error",

      // クラスメソッドがthisを使用していない場合に警告
      "class-methods-use-this": "warn",
      // 関数の循環的複雑度を10以下に制限
      complexity: ["error", 10],
      // ブロックの中括弧を強制
      curly: ["error", "all"],
      // switch文のdefaultを必須に
      "default-case": "error",
      // switch文のdefaultの位置を最後に
      "default-case-last": "error",
      // 引数のデフォルト値の位置を最後に
      "default-param-last": "error",
      // 型安全な等号演算子を強制
      eqeqeq: ["error", "always"],
      // getter/setterのペアを強制
      "grouped-accessor-pairs": "error",
      // ネストの深さを4以下に制限
      "max-depth": ["error", 4],
      // 1つのファイルの行数を300以下に制限
      "max-lines": ["error", { max: 300 }],
      // コールバックのネストを4以下に制限
      "max-nested-callbacks": ["error", 4],
      // alert, confirm, promptの使用を警告
      "no-alert": "warn",
      // arguments.callerとarguments.calleeを禁止
      "no-caller": "error",
      // コンソールの使用を警告
      "no-console": "warn",
      // 正規表現リテラルの先頭のスラッシュの後に等号をつけてはならない
      "no-div-regex": "error",
      // eval()の使用を禁止
      "no-eval": "error",
      // native typesの拡張を禁止
      "no-extend-native": "error",
      // 不必要なbind()を禁止
      "no-extra-bind": "error",
      // 省略形の型変換を許可しない
      "no-implicit-coercion": "error",
      // thisの値が未定義であるコンテキストでのthisの使用を禁止
      "no-invalid-this": "error",
      // __iterator__の使用を禁止
      "no-iterator": "error",
      // 変数と名前を共有するラベルを禁止
      "no-label-var": "error",
      // 三項演算子の入れ子を禁止
      "no-nested-ternary": "error",
      // 代入や比較以外のnewを禁止
      "no-new": "error",
      // Functionオブジェクトのnewを禁止
      "no-new-func": "error",
      // String, Number, Booleanのnewを禁止
      "no-new-wrappers": "error",
      // Objectのコンストラクタを引数なしで呼び出すのを禁止
      "no-object-constructor": "error",
      // 文字列リテラルにおける8進数エスケープシーケンスを禁止
      "no-octal-escape": "error",
      // 関数の引数への再代入を禁止
      "no-param-reassign": "error",
      // __proto__の使用を禁止
      "no-proto": "error",
      // return文での代入を禁止
      "no-return-assign": "error",
      // javascript:のURLを禁止
      "no-script-url": "error",
      // カンマ演算子を禁止
      "no-sequences": "error",
      // 外部スコープで宣言された変数のシャドウイングを禁止
      "no-shadow": "error",
      // アンダースコア始まりの変数名を禁止
      "no-underscore-dangle": "error",
      // 不必要な三項演算子をエラーとして報告
      "no-unneeded-ternary": "error",
      // .call()や.apply()の不要な呼び出しを禁止
      "no-useless-call": "error",
      // 計算されたプロパティ・キーの不必要な使用を禁止
      "no-useless-computed-key": "error",
      // import, export, 構造化解除された割り当ての同じ名前への変更を禁止
      "no-useless-rename": "error",
      // 冗長なreturn文をエラーとして報告
      "no-useless-return": "error",
      // varの使用を禁止
      "no-var": "error",
      // Math.pow()の代わりに指数演算子を使用
      "prefer-exponentiation-operator": "error",
      // 正規表現でnamed capture groupを強制
      "prefer-named-capture-group": "error",
      // オブジェクトリテラルを第1引数に持つObject.assign()の使用を禁止
      "prefer-object-spread": "error",
      // argumentsの代わりにRest Parametersを使用
      "prefer-rest-params": "error",
      // .apply()の代わりにスプレッド演算子を使用
      "prefer-spread": "error",
      // 文字列連結の代わりにテンプレートリテラルを使用
      "prefer-template": "error",
      // parseInt()の基数引数を強制
      radix: "error",
      // 正規表現でuまたはvフラグの使用を強制
      "require-unicode-regexp": "error",
      // Symbolに必ずdescriptionを付与
      "symbol-description": "error",

      // UnicodeのBOMを禁止
      "unicode-bom": ["error", "never"],

      /*
       * 以下はtseslint.configs.strictTypeCheckedおよび
       * tseslint.configs.stylisticTypeCheckedにないルール
       */

      // 型をimportする際の記述を統一
      "@typescript-eslint/consistent-type-imports": "error",
      // 関数の戻り値の型の明示的指定を強制
      "@typescript-eslint/explicit-function-return-type": "error",
      // クラスメンバーのアクセス修飾子を強制
      "@typescript-eslint/explicit-member-accessibility": "error",
      // 外部公開関数に型引数と戻り値の型注釈を強制
      "@typescript-eslint/explicit-module-boundary-types": "error",
      // 変数の宣言時に初期化を強制
      "init-declarations": "off",
      "@typescript-eslint/init-declarations": "error",
      // Enumの値指定を強制
      "@typescript-eslint/prefer-enum-initializers": "error",
      // boolean への型強制を禁止
      "@typescript-eslint/strict-boolean-expressions": "error",
      // Unionの全ケースを網羅しているかチェック
      "@typescript-eslint/switch-exhaustiveness-check": "error",
    },
  },
]);
