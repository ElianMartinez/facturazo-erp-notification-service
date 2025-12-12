
#set page(
  paper: "us-letter",
  margin: (top: 1.5cm, bottom: 1.5cm, left: 1.5cm, right: 1.5cm),
)

#set text(font: "Arial", size: 10pt)

#align(center)[
  #text(size: 16pt, weight: "bold")[FACTURA FISCAL ELECTRÓNICA]
]

#grid(
  columns: (1fr, 1fr),
  align: (left, right),
  [
    *Fecha:* 12/12/2025

    *Cliente:* Cliente Ejemplo

    *RNC:* 123456789
  ],
  [
    *NCF:* E31000000001

    *Válido hasta:* 31/12/2025

    *Código QR:* Verificación DGII
  ]
)

#table(
  columns: (1fr, 3fr, 1fr, 1fr, 1fr),
  stroke: 0.5pt + gray,
  inset: 8pt,

  table.header(
    [*Cant.*], [*Descripción*], [*Precio*], [*ITBIS*], [*Total*],
  ),

  [1], [Servicio de Desarrollo Web], [RD$ 3,000.00], [RD$ 540.00], [RD$ 3,540.00],
)

#align(right)[
  #table(
    columns: (150pt, 100pt),
    stroke: none,
    align: (right, right),
    [*Subtotal:*], [RD$ 3,000.00],
    [*ITBIS (18%):*], [RD$ 540.00],
    table.hline(stroke: 1.5pt),
    [*Total:*], [*RD$ 3,540.00*],
  )
]

#place(
  center + horizon,
  float: true,
  clearance: 3em,
  text(size: 72pt, fill: rgb(255, 0, 0, 30), weight: "bold")[PAGADA]
)
