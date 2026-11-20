# Minesweeper / Сапёр

## EN

A stateful WASM block for AIOS: a tiny console minesweeper with flood fill,
flags and 1-based coordinates. The board state lives in guest memory, so
several `block run` calls continue the same game. Source: `src/lib.rs`.

Run it from the AIOS text console after installing the block:

```
store source local:<path-to-aios-official-store>
store install minesweeper
block run minesweeper new 9 9 10
block run minesweeper open 1 1
block run minesweeper flag 5 5
block run minesweeper show
block run minesweeper help
```

Commands: `help`, `new [width height mines]`, `open <x> <y>`,
`flag <x> <y>`, `show`, `status`, `reset`. The field is printed as `.` for
covered cells, `F` for flags, `*` for mines, `-` for an empty revealed cell
and a digit for the number of adjacent mines. `open` creates a default 9x9/10
board if no game is active, and the first opened cell is never a mine.
Coordinates start at 1. Zoom keys such as `w`, `a`, `s`, `d` are not used —
the game is controlled by text commands only.

## RU

WASM-блок AIOS с состоянием: мини-сапёр с раскрытием пустых областей, флагами
и координатами с единицы. Состояние доски хранится в памяти гостя, поэтому
несколько вызовов `block run` продолжают одну и ту же игру. Исходник:
`src/lib.rs`.

Запуск из текстовой консоли AIOS после установки блока:

```
store source local:<путь-к-aios-official-store>
store install minesweeper
block run minesweeper new 9 9 10
block run minesweeper open 1 1
block run minesweeper flag 5 5
block run minesweeper show
block run minesweeper help
```

Команды: `help`, `new [ширина высота мин]`, `open <x> <y>`,
`flag <x> <y>`, `show`, `status`, `reset`. Доска выводится так: `.` — закрытая
клетка, `F` — флаг, `*` — мина, `-` — пустая раскрытая клетка, цифра — число
соседних мин. Если игра не создана, `open` автоматически создаёт доску 9x9
с 10 минами, причём первая открытая клетка никогда не содержит мину.
Координаты начинаются с 1. Управление только текстовыми командами, клавиши
WASD не используются.
