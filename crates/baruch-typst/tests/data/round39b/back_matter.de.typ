== before
= Der Platz

Am Bach steht eine #link(<baruch-gl-7b0e5c1d-0000-4000-8000-000000000039>)[#emph[Weide]], wie Ps 46,4#metadata((register: "bibel", term: "Psalmen", sub: "46,4", sort: "19.046.004-046.004"))<baruch-idx> sagt.#metadata((register: "stichwort", term: "Lehm"))<baruch-idx>
= Das Fundament

Der #link(<baruch-gl-7b0e5c1d-0000-4000-8000-000000000040>)[#emph[Lehm]] trägt schlecht.
// Baruch: Register und Glossar
#let baruch-idx-ranges(ns) = {
  let ns = ns.sorted().dedup()
  let out = ()
  let i = 0
  while i < ns.len() {
    let a = ns.at(i)
    let b = a
    while i + 1 < ns.len() and ns.at(i + 1) == b + 1 {
      i += 1
      b = ns.at(i)
    }
    out.push(if b > a { str(a) + "–" + str(b) } else { str(a) })
    i += 1
  }
  out.join(", ")
}
#let baruch-idx-pages() = {
  let d = (:)
  for m in query(<baruch-idx>) {
    let v = m.value
    if type(v) == dictionary and v.at("see", default: "") == "" {
      let k = v.at("register", default: "") + "\u{1f}" + v.at("term", default: "") + "\u{1f}" + v.at("sub", default: "")
      d.insert(k, d.at(k, default: ()) + (counter(page).at(m.location()).first(),))
    }
  }
  d
}
#let baruch-idx-group(letter) = block(above: 0.9em, below: 0.35em, sticky: true, strong(letter))
#let baruch-idx-entry(d, k, term, level, see, also) = {
  let ns = d.at(k, default: ())
  let parts = ()
  if ns.len() > 0 { parts.push([#baruch-idx-ranges(ns)]) }
  if see.len() > 0 { parts.push([#emph[siehe] #see.join("; ")]) }
  if also.len() > 0 { parts.push([#emph[siehe auch] #also.join("; ")]) }
  block(above: 0.3em, below: 0.3em, inset: (left: level * 1.2em), breakable: true,
    par(first-line-indent: 0em, hanging-indent: 1.2em, justify: false,
      [#term] + if parts.len() > 0 { [ ] + parts.join([; ]) } else { [] }))
}
#let baruch-gl-entry(anchor, term, variants, short, long, refs) = block(above: 0.6em, below: 0.9em, breakable: true, {
  set par(first-line-indent: 0em, justify: false)
  par[#anchor#strong(term)#if variants.len() > 0 [ (#variants.join(", "))]#if short != "" [ – #short]]
  for p in long { par(p) }
  if refs.len() > 0 { par[#emph[Bibelstellen:] #refs.join("; ")] }
})

#pagebreak(weak: true)
#heading(level: 1, numbering: none)[#"Glossar"]
#baruch-gl-entry([#metadata("baruch-glossar")<baruch-gl-7b0e5c1d-0000-4000-8000-000000000040>], "Lehm", (), "Schwerer Boden.", (), ())
#baruch-gl-entry([#metadata("baruch-glossar")<baruch-gl-7b0e5c1d-0000-4000-8000-000000000039>], "Weide", ("Salix",), "Baum am Wasser.", ("Sie wächst schnell.", "Ihre Zweige biegen sich."), ("Ps 137,2",))

#pagebreak(weak: true)
#heading(level: 1, numbering: none)[#"Stichwortregister"]
#context {
  let d = baruch-idx-pages()
  let body = {
    baruch-idx-group("L")
    baruch-idx-entry(d, "stichwort\u{1f}Lehm\u{1f}", "Lehm", 0, (), ())
  }
  columns(2, gutter: 1.5em, body)
}

#pagebreak(weak: true)
#heading(level: 1, numbering: none)[#"Bibelstellenregister"]
#context {
  let d = baruch-idx-pages()
  let body = {
    baruch-idx-entry(d, "bibel\u{1f}Psalmen\u{1f}", "Psalmen", 0, (), ())
    baruch-idx-entry(d, "bibel\u{1f}Psalmen\u{1f}46,4", "46,4", 1, (), ())
  }
  columns(2, gutter: 1.5em, body)
}

#pagebreak(weak: true)
#heading(level: 1, numbering: none)[#"Personenregister"]
#emph("Keine Einträge.")

== after
= Der Platz

Am Bach steht eine #link(<baruch-gl-7b0e5c1d-0000-4000-8000-000000000039>)[#emph[Weide]], wie Ps 46,4 sagt.#metadata((register: "stichwort", term: "Lehm"))<baruch-idx>
= Das Fundament

Der #link(<baruch-gl-7b0e5c1d-0000-4000-8000-000000000040>)[#emph[Lehm]] trägt schlecht.
// Baruch: Register und Glossar
#let baruch-idx-ranges(ns) = {
  let ns = ns.sorted().dedup()
  let out = ()
  let i = 0
  while i < ns.len() {
    let a = ns.at(i)
    let b = a
    while i + 1 < ns.len() and ns.at(i + 1) == b + 1 {
      i += 1
      b = ns.at(i)
    }
    out.push(if b > a { str(a) + "–" + str(b) } else { str(a) })
    i += 1
  }
  out.join(", ")
}
#let baruch-idx-pages() = {
  let d = (:)
  for m in query(<baruch-idx>) {
    let v = m.value
    if type(v) == dictionary and v.at("see", default: "") == "" {
      let k = v.at("register", default: "") + "\u{1f}" + v.at("term", default: "") + "\u{1f}" + v.at("sub", default: "")
      d.insert(k, d.at(k, default: ()) + (counter(page).at(m.location()).first(),))
    }
  }
  d
}
#let baruch-idx-group(letter) = block(above: 0.9em, below: 0.35em, sticky: true, strong(letter))
#let baruch-idx-entry(d, k, term, level, see, also) = {
  let ns = d.at(k, default: ())
  let parts = ()
  if ns.len() > 0 { parts.push([#baruch-idx-ranges(ns)]) }
  if see.len() > 0 { parts.push([#emph[siehe] #see.join("; ")]) }
  if also.len() > 0 { parts.push([#emph[siehe auch] #also.join("; ")]) }
  block(above: 0.3em, below: 0.3em, inset: (left: level * 1.2em), breakable: true,
    par(first-line-indent: 0em, hanging-indent: 1.2em, justify: false,
      [#term] + if parts.len() > 0 { [ ] + parts.join([; ]) } else { [] }))
}
#let baruch-gl-entry(anchor, term, variants, short, long, refs) = block(above: 0.6em, below: 0.9em, breakable: true, {
  set par(first-line-indent: 0em, justify: false)
  par[#anchor#strong(term)#if variants.len() > 0 [ (#variants.join(", "))]#if short != "" [ – #short]]
  for p in long { par(p) }
  if refs.len() > 0 { par[#emph[Bibelstellen:] #refs.join("; ")] }
})

#pagebreak(weak: true)
#heading(level: 1, numbering: none)[#"Personenregister"]
#emph("Keine Einträge.")

#pagebreak(weak: true)
#heading(level: 1, numbering: none)[#"Glossar"]
#baruch-gl-entry([#metadata("baruch-glossar")<baruch-gl-7b0e5c1d-0000-4000-8000-000000000040>], "Lehm", (), "Schwerer Boden.", (), ())
#baruch-gl-entry([#metadata("baruch-glossar")<baruch-gl-7b0e5c1d-0000-4000-8000-000000000039>], "Weide", ("Salix",), "Baum am Wasser.", ("Sie wächst schnell.", "Ihre Zweige biegen sich."), ("Ps 137,2",))

