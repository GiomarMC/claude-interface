// Conversaciones de ejemplo para las capturas del README. Cada paso es una
// línea del protocolo stream-json de Claude Code, igual que las que emite el CLI.

export const CWD = "/home/dev/proyectos/tienda-api";
const FILE = `${CWD}/src/routes/orders.ts`;

export const ORDERS_TS = `import { Router } from "express";
import { db } from "../db";

export const orders = Router();

export function calculateTotal(cart: Cart) {
  const currency = cart.items[0].price.currency;
  const amount = cart.items.reduce((sum, i) => sum + i.price.amount * i.qty, 0);
  return { amount, currency };
}

orders.post("/", async (req, res) => {
  const cart = await db.carts.find(req.body.cartId);
  const total = calculateTotal(cart);
  const order = await db.orders.create({ cartId: cart.id, total });
  res.status(201).json(order);
});
`;

/** Miniatura de "captura de error" para el adjunto de imagen. */
export const PNG =
  "iVBORw0KGgoAAAANSUhEUgAAAGAAAABAEAIAAAA6xjkaAAAAIGNIUk0AAHomAACAhAAA+gAAAIDoAAB1MAAA6mAAADqYAAAXcJy6UTwAAAAGYktHRP///////wlY99wAAAAHdElNRQfqCh0BCi/DNLpDAAAC7UlEQVR42u3ZT0iTYRzA8efd1lyaHWbKTB0UURCsIPJgrUCQDnaRjBTBQUaHlkHSyXkLqkMjJfSF0C4TygkVnotGMFinzA5BIkiopAxHyz8b2WYXDxu6w/ZT3kbfz/HhecfDy5cf2zPN5XK5XC4FFMRk9AFQ3AgIIjkDsp7SejV3X0ndZcfwlztnDp+s6pupa3Q8tZ7VfNrFrJ3HLfXK5utpjSh9qru/WdN8l1qDSre6LOeULetDpy0Dqj9l7/hUdnQjNdxSUZqq6HhT5lTTlkE1YPSrQCG0XN+BfOG6844n7QcrE/aRzPVgbbQi1v3APvd20bu183TrM6W3Bdzr2u2snY/CJzaHHo69alZbO1OHOp6X1aZ/NKZticydJkfoSPKAeflF09p3o18I8pNzArW/q5yz+7evt81XLtsHs1Za3O+z09la73V/y15PLzV+sP3cvjO92DhrWzX6VaAQOQMK3ogei/m2r48tRktiXVk7J8Ntm0M7fMLVsDl73VQVupa073CI6tBG0qZQhHIG9Lhpvn7p5diPqDV2XU2qVbU+loiWxm76b81fWHqdudM/NXFf9QSvhE2bQ+qzmlUq2BUu3xzyJyZGVE/mTvPH8a9rd03Vod/JEqWpGfXHVB3al9xvjoz/Wrtn9KtAITTugSDBz3iIEBBELPF4PB6PG30MFCsmEEQICCIEBBECgggBQYSAIGLJ94FAoKamvNzoYxcrj2dhYWXF6FPsJiYQRAgIIgQEEQKCCAFBhIAgojmdTqfTafQxUKyYQBAhIIgQEEQICCJ5/xfm9Xq9Xq/Rx8bu03Vd1/V8n2ICQYSAIEJAEOEiESJMIIgQEEQICCLcAxWlwu5s9gITCCIEBBECggj3QBBhAkGEgCBCQBDhHui/sHf3RkwgiBAQRAgIItwDQYQJBBECgggBQSTve6BIJBDo7DT62EZqaPB4RkeNPsW/ggkEEQKCCAFBhHsgiDCBIEJAECEgiBAQRAgIIn8BfTm8EeluGRMAAAAldEVYdGRhdGU6Y3JlYXRlADIwMjYtMTAtMjlUMDE6MTA6NDcrMDA6MDBPuK8cAAAAJXRFWHRkYXRlOm1vZGlmeQAyMDI2LTEwLTI5VDAxOjEwOjQ3KzAwOjAwPuUXoAAAACh0RVh0ZGF0ZTp0aW1lc3RhbXAAMjAyNi0xMC0yOVQwMToxMDo0NyswMDowMGnwNn8AAAAASUVORK5CYII=";

const init = {
  type: "system",
  subtype: "init",
  session_id: "demo",
  model: "claude-opus-5-5",
  claude_code_version: "2.1.312",
  permissionMode: "acceptEdits",
  output_style: "default",
};

const assistant = (...content: any[]) => ({
  type: "assistant",
  parent_tool_use_id: null,
  message: { role: "assistant", content },
});

const toolResult = (id: string, text: string, meta?: any, isError = false) => ({
  type: "user",
  parent_tool_use_id: null,
  tool_use_result: meta,
  message: { role: "user", content: [{ type: "tool_result", tool_use_id: id, content: text, is_error: isError }] },
});

const result = (ms: number) => ({
  type: "result",
  subtype: "success",
  is_error: false,
  duration_ms: ms,
  duration_api_ms: Math.round(ms * 0.6),
  total_cost_usd: 0.4821,
  modelUsage: { "claude-opus-5-5": { contextWindow: 1_000_000 } },
});

const rateLimits = {
  type: "rate_limit_event",
  rate_limit_info: {
    unifiedWindows: {
      five_hour: { utilization: 0.23, resetsAt: Math.floor(Date.now() / 1000) + 3 * 3600 },
      seven_day: { utilization: 0.41, resetsAt: Math.floor(Date.now() / 1000) + 4 * 86400 },
    },
  },
};

export const FIX_PROMPT =
  "El endpoint POST /orders devuelve 500 cuando el carrito está vacío (adjunto el archivo y la captura del error). ¿Puedes arreglarlo y añadir un test?";

export const FIX_TURN: any[] = [
  init,
  rateLimits,
  assistant({
    type: "text",
    text:
      "En la captura se ve `TypeError: Cannot read properties of undefined (reading 'price')`. " +
      "`calculateTotal` lee `cart.items[0]` sin comprobar que el carrito tenga productos. " +
      "Como ya me pasaste el archivo, no necesito buscarlo: lo corrijo directamente.",
  }),
  assistant({
    type: "tool_use",
    id: "t1",
    name: "Edit",
    input: {
      file_path: FILE,
      old_string: "",
      new_string: "",
    },
  }),
  toolResult("t1", "The file has been updated.", {
    structuredPatch: [
      {
        oldStart: 6,
        newStart: 6,
        lines: [
          " export function calculateTotal(cart: Cart) {",
          "-  const currency = cart.items[0].price.currency;",
          "+  if (cart.items.length === 0) {",
          "+    throw new HttpError(422, \"El carrito está vacío\");",
          "+  }",
          "+  const currency = cart.items[0].price.currency;",
          "   const amount = cart.items.reduce((sum, i) => sum + i.price.amount * i.qty, 0);",
        ],
      },
    ],
  }),
  assistant({
    type: "tool_use",
    id: "t2",
    name: "Write",
    input: {
      file_path: `${CWD}/tests/orders.test.ts`,
      content: `import { request } from "./helpers";

test("rechaza carritos vacíos con 422", async () => {
  const cart = await request.post("/carts").send({ items: [] });
  const res = await request.post("/orders").send({ cartId: cart.body.id });
  expect(res.status).toBe(422);
  expect(res.body.error).toBe("El carrito está vacío");
});
`,
    },
  }),
  toolResult("t2", "File created successfully."),
  assistant({
    type: "tool_use",
    id: "t3",
    name: "Bash",
    input: { command: "pnpm test orders", description: "Ejecuta los tests de pedidos" },
  }),
  toolResult(
    "t3",
    [
      " ✓ tests/orders.test.ts (4 tests) 212ms",
      "   ✓ crea un pedido con total correcto",
      "   ✓ rechaza carritos vacíos con 422",
      "   ✓ calcula el total con varias cantidades",
      "   ✓ mantiene la moneda del carrito",
      "",
      " Test Files  1 passed (1)",
      "      Tests  4 passed (4)",
    ].join("\n"),
  ),
  assistant({
    type: "text",
    text: [
      "Listo. El pedido ya no falla con un carrito vacío:",
      "",
      "- **`src/routes/orders.ts`**: `calculateTotal` responde **422** con un mensaje claro en vez de lanzar un error 500.",
      "- **`tests/orders.test.ts`**: nuevo test que cubre el caso; los 4 tests pasan.",
      "",
      "Si el frontend quiere mostrar el error, la respuesta queda así:",
      "",
      "```json",
      '{ "error": "El carrito está vacío" }',
      "```",
    ].join("\n"),
  }),
  result(48_000),
];

export const PUSH_PROMPT = "Perfecto, sube la rama y abre el PR";

export const PUSH_TURN: any[] = [
  assistant({ type: "text", text: "Hago push de `fix/carrito-vacio` y luego abro el PR con `gh`." }),
  assistant({
    type: "tool_use",
    id: "t4",
    name: "Bash",
    input: { command: "git push -u origin fix/carrito-vacio", description: "Sube la rama al remoto" },
  }),
  {
    type: "control_request",
    request_id: "perm-1",
    request: {
      subtype: "can_use_tool",
      tool_name: "Bash",
      input: { command: "git push -u origin fix/carrito-vacio", description: "Sube la rama al remoto" },
      permission_suggestions: [
        {
          type: "addRules",
          rules: [{ toolName: "Bash", ruleContent: "git push:*" }],
          behavior: "allow",
          destination: "localSettings",
        },
      ],
      tool_use_id: "t4",
    },
  },
];
