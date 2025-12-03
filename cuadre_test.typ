#set document(title: "Cuadre de Caja - SQ-138", author: "LE CROISSANT DORE")
#set page(
  paper: "a4",
  margin: (top: 16pt, bottom: 14pt, left: 20pt, right: 20pt),
)
#set text(font: "Inter", size: 8pt, fill: rgb("1E293B"))

// ============================================
// HEADER
// ============================================
#grid(
  columns: (1fr, auto),
  align: (left, right),
  [
    #text(size: 18pt, weight: "bold")[Cuadre de Caja]
    #v(2pt)
    #text(size: 11pt, weight: "semibold", fill: rgb("2563EB"))[LE CROISSANT DORE]
    #v(1pt)
    #text(size: 7pt, fill: rgb("64748B"))[RNC: 101707399 · Enriquillo, No. 25]
  ],
  [
    #text(size: 16pt, weight: "bold")[SQ-138]
    #v(2pt)
    #text(size: 7pt, fill: rgb("64748B"))[12/02/2025, 4:00 a.m.]
  ]
)

#v(6pt)
#line(length: 100%, stroke: 2pt + rgb("2563EB"))
#v(8pt)

// ============================================
// INFO BAR - 3 boxes igual altura
// ============================================
#grid(
  columns: (1fr, 1fr, 1fr),
  gutter: 10pt,
  rect(
    width: 100%,
    height: 44pt,
    fill: rgb("F8FAFC"),
    stroke: 1pt + rgb("E2E8F0"),
    radius: 5pt,
    inset: (x: 10pt, y: 8pt),
    [
      #text(size: 6pt, weight: "semibold", fill: rgb("64748B"), tracking: 0.5pt)[CAJERO]
      #v(4pt)
      #text(size: 11pt, weight: "semibold")[Caja 4]
    ]
  ),
  rect(
    width: 100%,
    height: 44pt,
    fill: rgb("F8FAFC"),
    stroke: 1pt + rgb("E2E8F0"),
    radius: 5pt,
    inset: (x: 10pt, y: 8pt),
    [
      #text(size: 6pt, weight: "semibold", fill: rgb("64748B"), tracking: 0.5pt)[TRANSACCIONES]
      #v(4pt)
      #text(size: 11pt, weight: "semibold")[21]
    ]
  ),
  rect(
    width: 100%,
    height: 44pt,
    fill: rgb("F8FAFC"),
    stroke: 1pt + rgb("E2E8F0"),
    radius: 5pt,
    inset: (x: 10pt, y: 8pt),
    [
      #text(size: 6pt, weight: "semibold", fill: rgb("64748B"), tracking: 0.5pt)[ESTADO]
      #v(4pt)
      #box(
        fill: rgb("ECFDF5"),
        radius: 10pt,
        inset: (x: 8pt, y: 3pt),
        [#text(size: 8pt, weight: "semibold", fill: rgb("059669"))[● SOBRANTE]]
      )
    ]
  ),
)

#v(10pt)

// ============================================
// MAIN CONTENT - TWO COLUMNS
// ============================================
#grid(
  columns: (1fr, 1fr),
  gutter: 12pt,

  // ========== LEFT COLUMN ==========
  [
    // INGRESOS
    #text(size: 9pt, weight: "bold")[INGRESOS]
    #v(5pt)

    #table(
      columns: (1fr, 50pt, 70pt),
      stroke: 1pt + rgb("E2E8F0"),
      inset: 6pt,
      fill: (col, row) => if row == 0 { rgb("F8FAFC") } else { white },
      align: (col, row) => if col == 0 { left } else if col == 1 { center } else { right },
      [#text(size: 6pt, weight: "semibold", fill: rgb("64748B"))[CONCEPTO]],
      [#text(size: 6pt, weight: "semibold", fill: rgb("64748B"))[CANT.]],
      [#text(size: 6pt, weight: "semibold", fill: rgb("64748B"))[MONTO]],
      [POS], [21], [\$28,444.00],
      [Recibos], [0], [\$0.00],
      [Efectivo Inicial], [—], [\$0.00],
    )

    #v(4pt)
    #rect(
      width: 100%,
      fill: rgb("2563EB"),
      radius: 5pt,
      inset: (x: 10pt, y: 7pt),
      [
        #grid(
          columns: (1fr, auto),
          align: (left, right),
          [#text(size: 9pt, weight: "semibold", fill: white)[TOTAL INGRESOS]],
          [#text(size: 12pt, weight: "bold", fill: white)[\$28,444.00]]
        )
      ]
    )

    #v(10pt)

    // EGRESOS
    #text(size: 9pt, weight: "bold")[EGRESOS]
    #v(5pt)

    #table(
      columns: (1fr, 50pt, 70pt),
      stroke: 1pt + rgb("E2E8F0"),
      inset: 6pt,
      fill: (col, row) => if row == 0 { rgb("F8FAFC") } else { white },
      align: (col, row) => if col == 0 { left } else if col == 1 { center } else { right },
      [#text(size: 6pt, weight: "semibold", fill: rgb("64748B"))[CONCEPTO]],
      [#text(size: 6pt, weight: "semibold", fill: rgb("64748B"))[CANT.]],
      [#text(size: 6pt, weight: "semibold", fill: rgb("64748B"))[MONTO]],
      [Comprobantes], [0], [\$0.00],
      [Pagos con NC], [0], [\$0.00],
    )

    #v(4pt)
    #rect(
      width: 100%,
      fill: rgb("475569"),
      radius: 5pt,
      inset: (x: 10pt, y: 7pt),
      [
        #grid(
          columns: (1fr, auto),
          align: (left, right),
          [#text(size: 9pt, weight: "semibold", fill: white)[TOTAL EGRESOS]],
          [#text(size: 12pt, weight: "bold", fill: white)[\$0.00]]
        )
      ]
    )
  ],

  // ========== RIGHT COLUMN ==========
  [
    // RESUMEN GENERAL
    #rect(
      width: 100%,
      stroke: 1pt + rgb("E2E8F0"),
      radius: 5pt,
      inset: 0pt,
      [
        #box(
          width: 100%,
          fill: white,
          inset: (x: 10pt, y: 7pt),
          [#text(size: 9pt, weight: "bold")[RESUMEN GENERAL]]
        )
        #line(length: 100%, stroke: 1pt + rgb("E2E8F0"))
        #box(
          width: 100%,
          fill: rgb("F8FAFC"),
          inset: 10pt,
          [
            #grid(
              columns: (1fr, auto),
              row-gutter: 5pt,
              [#text(size: 8pt, fill: rgb("64748B"))[Total Ingresos]],
              [#text(size: 8pt, weight: "semibold")[\$28,444.00]],
              [#text(size: 8pt, fill: rgb("64748B"))[Total Egresos]],
              [#text(size: 8pt, weight: "semibold")[\$0.00]],
            )
            #v(5pt)
            #rect(
              width: 100%,
              fill: rgb("EFF6FF"),
              radius: 4pt,
              inset: (x: 10pt, y: 6pt),
              [
                #grid(
                  columns: (1fr, auto),
                  align: (left, right),
                  [#text(size: 8pt, weight: "semibold")[Total a Cuadrar]],
                  [#text(size: 10pt, weight: "bold", fill: rgb("2563EB"))[\$28,444.00]]
                )
              ]
            )
            #v(5pt)
            #grid(
              columns: (1fr, auto),
              [#text(size: 8pt, fill: rgb("64748B"))[Total en Caja]],
              [#text(size: 8pt, weight: "semibold")[\$29,120.27]],
            )
          ]
        )
      ]
    )

    #v(8pt)

    // DESGLOSE DE EFECTIVO
    #rect(
      width: 100%,
      stroke: 1pt + rgb("E2E8F0"),
      radius: 5pt,
      inset: 0pt,
      [
        #box(
          width: 100%,
          fill: white,
          inset: (x: 10pt, y: 7pt),
          [#text(size: 9pt, weight: "bold")[DESGLOSE DE EFECTIVO]]
        )
        #line(length: 100%, stroke: 1pt + rgb("E2E8F0"))
        #box(
          width: 100%,
          fill: rgb("F8FAFC"),
          inset: 8pt,
          [
            #grid(
              columns: (1fr, 1fr),
              gutter: 6pt,
              rect(width: 100%, stroke: 1pt + rgb("E2E8F0"), radius: 3pt, inset: 6pt, fill: white,
                grid(columns: (1fr, auto), align: (left, right),
                  [#text(size: 7pt, fill: rgb("64748B"))[\$1,000 × 2]],
                  [#text(size: 8pt, weight: "semibold")[\$2,000.00]]
                )
              ),
              rect(width: 100%, stroke: 1pt + rgb("E2E8F0"), radius: 3pt, inset: 6pt, fill: white,
                grid(columns: (1fr, auto), align: (left, right),
                  [#text(size: 7pt, fill: rgb("64748B"))[\$100 × 3]],
                  [#text(size: 8pt, weight: "semibold")[\$300.00]]
                )
              ),
              rect(width: 100%, stroke: 1pt + rgb("E2E8F0"), radius: 3pt, inset: 6pt, fill: white,
                grid(columns: (1fr, auto), align: (left, right),
                  [#text(size: 7pt, fill: rgb("64748B"))[\$50 × 2]],
                  [#text(size: 8pt, weight: "semibold")[\$100.00]]
                )
              ),
              rect(width: 100%, stroke: 1pt + rgb("E2E8F0"), radius: 3pt, inset: 6pt, fill: white,
                grid(columns: (1fr, auto), align: (left, right),
                  [#text(size: 7pt, fill: rgb("64748B"))[\$5 × 1]],
                  [#text(size: 8pt, weight: "semibold")[\$5.00]]
                )
              ),
            )
            #v(6pt)
            #rect(
              width: 100%,
              fill: rgb("EFF6FF"),
              stroke: 1pt + rgb("BFDBFE"),
              radius: 4pt,
              inset: (x: 10pt, y: 6pt),
              [
                #grid(
                  columns: (1fr, auto),
                  align: (left, right),
                  [#text(size: 7pt, weight: "semibold", fill: rgb("2563EB"))[TOTAL EFECTIVO]],
                  [#text(size: 10pt, weight: "bold", fill: rgb("2563EB"))[\$2,405.00]]
                )
              ]
            )
          ]
        )
      ]
    )
  ]
)

#v(10pt)

// ============================================
// CUADRE POR MÉTODO DE PAGO
// ============================================
#text(size: 9pt, weight: "bold")[CUADRE POR MÉTODO DE PAGO]
#v(5pt)

#table(
  columns: (1fr, 1fr, 1fr, 80pt),
  stroke: 1pt + rgb("E2E8F0"),
  inset: 7pt,
  fill: (col, row) => if row == 0 { rgb("F8FAFC") } else { white },
  align: (col, row) => if col == 0 { left } else { center },
  [#text(size: 6pt, weight: "semibold", fill: rgb("64748B"))[MÉTODO]],
  [#text(size: 6pt, weight: "semibold", fill: rgb("64748B"))[SISTEMA]],
  [#text(size: 6pt, weight: "semibold", fill: rgb("64748B"))[EN CAJA]],
  [#text(size: 6pt, weight: "semibold", fill: rgb("64748B"))[DIFERENCIA]],
  [Efectivo], [\$2,392.70], [\$2,405.00], [#text(fill: rgb("059669"), weight: "semibold")[+\$12.30]],
  [Cheque], [\$0.00], [\$0.00], [#text(fill: rgb("64748B"))[\$0.00]],
  [Tarjeta], [\$26,715.24], [\$26,715.27], [#text(fill: rgb("059669"), weight: "semibold")[+\$0.03]],
  [Transferencia], [\$0.00], [\$0.00], [#text(fill: rgb("64748B"))[\$0.00]],
)

#rect(
  width: 100%,
  fill: rgb("2563EB"),
  inset: 7pt,
  [
    #grid(
      columns: (1fr, 1fr, 1fr, 80pt),
      align: (left, center, center, center),
      [#text(weight: "bold", fill: white)[Total]],
      [#text(weight: "bold", fill: white)[\$29,107.94]],
      [#text(weight: "bold", fill: white)[\$29,120.27]],
      [#text(weight: "bold", fill: white)[+\$12.33]]
    )
  ]
)

#v(10pt)

// ============================================
// BALANCE FINAL
// ============================================
#block[
  #rect(
    width: 100%,
    fill: rgb("1E293B"),
    inset: (x: 16pt, y: 10pt),
    radius: (top-left: 6pt, top-right: 6pt, bottom-left: 0pt, bottom-right: 0pt),
    [
      #grid(
        columns: (1fr, auto),
        align: (left, right),
        [#text(size: 10pt, weight: "bold", fill: white)[BALANCE FINAL]],
        [#text(size: 22pt, weight: "bold", fill: rgb("4ADE80"))[+\$12.33]]
      )
    ]
  )
  #v(-1pt)
  #rect(
    width: 100%,
    fill: rgb("F8FAFC"),
    stroke: 1pt + rgb("E2E8F0"),
    inset: (x: 16pt, y: 10pt),
    radius: (top-left: 0pt, top-right: 0pt, bottom-left: 6pt, bottom-right: 6pt),
    [
      #grid(
        columns: (1fr, 1fr, 1fr),
        align: (left, center, right),
        [
          #box(width: 8pt, height: 8pt, fill: rgb("2563EB"), radius: 50%)
          #h(4pt)
          #text(size: 7pt, fill: rgb("64748B"))[Sistema:]
          #h(3pt)
          #text(size: 8pt, weight: "semibold")[\$29,107.94]
        ],
        [
          #box(width: 8pt, height: 8pt, fill: rgb("475569"), radius: 50%)
          #h(4pt)
          #text(size: 7pt, fill: rgb("64748B"))[En Caja:]
          #h(3pt)
          #text(size: 8pt, weight: "semibold")[\$29,120.27]
        ],
        [
          #box(
            fill: rgb("ECFDF5"),
            radius: 10pt,
            inset: (x: 10pt, y: 4pt),
            [#text(size: 7pt, weight: "semibold", fill: rgb("059669"))[● SOBRANTE DE \$12.33]]
          )
        ]
      )
    ]
  )
]

#v(8pt)

// ============================================
// FOOTER
// ============================================
#line(length: 100%, stroke: 1pt + rgb("E2E8F0"))
#v(5pt)
#grid(
  columns: (1fr, auto),
  align: (left, right),
  [#text(size: 6pt, fill: rgb("64748B"))[Documento generado por FACTURAZO · Sistema de Gestión]],
  [#text(size: 6pt, fill: rgb("64748B"))[Generado: 03/12/2025, 2:41 a.m.]]
)
