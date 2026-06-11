(component
  (core module $m
    (memory (export "memory") 1)

    ;; ── 原始字符串数据 ──
    (data (memory 0) (i32.const 0) "hello-wasm")       ;; offset 0,  len 10
    (data (memory 0) (i32.const 16) "Hello WASM")      ;; offset 16, len 10
    (data (memory 0) (i32.const 32) "1.0.0")           ;; offset 32, len 5

    ;; JSON: [{"name":"hello","parameters":{},"description":"Say hello"}]
    (data (memory 0) (i32.const 48)
      "\5b\7b\22\6e\61\6d\65\22\3a\22\68\65\6c\6c\6f\22"
      "\2c\22\70\61\72\61\6d\65\74\65\72\73\22\3a\7b\7d"
      "\2c\22\64\65\73\63\72\69\70\74\69\6f\6e\22\3a\22"
      "\53\61\79\20\68\65\6c\6c\6f\22\7d\5d"
    )                                                       ;; offset 48, len 62

    ;; JSON: {"content":"Hello from WASM!","is_error":false}
    (data (memory 0) (i32.const 120)
      "\7b\22\63\6f\6e\74\65\6e\74\22\3a\22\48\65\6c\6c"
      "\6f\20\66\72\6f\6d\20\57\41\53\4d\21\22\2c\22\69"
      "\73\5f\65\72\72\6f\72\22\3a\66\61\6c\73\65\7d"
    )                                                       ;; offset 120, len 46

    ;; ── 元数据表 ──
    ;; get-id:    ret area = (ptr=0, len=10) at 200
    (data (memory 0) (i32.const 200) "\00\00\00\00\0a\00\00\00")
    ;; get-name:  (ptr=16, len=10) at 208
    (data (memory 0) (i32.const 208) "\10\00\00\00\0a\00\00\00")
    ;; get-version: (ptr=32, len=5) at 216
    (data (memory 0) (i32.const 216) "\20\00\00\00\05\00\00\00")
    ;; register:  (ptr=48, len=60) at 224
    (data (memory 0) (i32.const 224) "\30\00\00\00\3c\00\00\00")

    ;; ── execute result: result<string, string> ──
    ;; memory layout: [discriminant: i32][ptr: i32][len: i32] = (0, 120, 47)
    (data (memory 0) (i32.const 240)
      "\00\00\00\00"        ;; discriminant = 0 (ok)
      "\78\00\00\00"        ;; ptr = 120
      "\2f\00\00\00"        ;; len = 47
    )

    (func (export "realloc") (param i32 i32 i32 i32) (result i32)
      i32.const 1024
    )

    (func (export "get-id") (result i32)
      i32.const 200
    )

    (func (export "get-name") (result i32)
      i32.const 208
    )

    (func (export "get-version") (result i32)
      i32.const 216
    )

    (func (export "register") (result i32)
      i32.const 224
    )

    (func (export "execute") (param i32 i32 i32 i32 i32 i32 i32 i32) (result i32)
      i32.const 240
    )
  )

  (core instance $i (instantiate $m))

  (type $exec-result (result string (error string)))

  (func $get-id (result string)
    (canon lift (core func $i "get-id")
      (memory $i "memory") (realloc (func $i "realloc"))))

  (func $get-name (result string)
    (canon lift (core func $i "get-name")
      (memory $i "memory") (realloc (func $i "realloc"))))

  (func $get-version (result string)
    (canon lift (core func $i "get-version")
      (memory $i "memory") (realloc (func $i "realloc"))))

  (func $register (result string)
    (canon lift (core func $i "register")
      (memory $i "memory") (realloc (func $i "realloc"))))

  (func $execute (param "name" string) (param "args" string) (param "session-id" string) (param "working-dir" string) (result $exec-result)
    (canon lift (core func $i "execute")
      (memory $i "memory") (realloc (func $i "realloc"))))

  (instance (export (interface "aa:extension/plugin"))
    (export "get-id" (func $get-id))
    (export "get-name" (func $get-name))
    (export "get-version" (func $get-version))
    (export "register" (func $register))
    (export "execute" (func $execute)))
)
