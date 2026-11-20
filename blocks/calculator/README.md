# Calculator / Калькулятор

## EN

A stateful WASM block for AIOS. It evaluates arithmetic expressions and keeps
`ans`, assigned variables and the last 20 calculations in guest memory. Source:
`src/lib.rs`.

Run it from the AIOS text console after installing the block:

```
store source local:<path-to-aios-official-store>
store install calculator
block run calculator 2+3*4
block run calculator ans+1
block run calculator width=3
block run calculator width*width
block run calculator history
```

Commands: `help`, `reset`, `history`, `ans`, `<expression>`, `<name> = <expression>`.
Operators: `+`, `-`, `*`, `/`, `%`, parentheses. Division by zero is an error.

## RU

Это WASM-блок AIOS с состоянием. Он вычисляет арифметические выражения и
хранит `ans`, присвоенные переменные и последние 20 вычислений в памяти гостя.
Исходник: `src/lib.rs`.

Запуск из текстовой консоли AIOS после установки блока:

```
store source local:<путь-к-aios-official-store>
store install calculator
block run calculator 2+3*4
block run calculator ans+1
block run calculator width=3
block run calculator width*width
block run calculator history
```

Команды: `help`, `reset`, `history`, `ans`, `<выражение>`, `<имя> = <выражение>`.
Операторы: `+`, `-`, `*`, `/`, `%`, скобки. Деление на ноль — ошибка.
