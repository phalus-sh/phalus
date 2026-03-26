# Ethics

PHALUS is a tool for research, education, and transparent discourse. This page sets out the ethical context it exists within, and the obligations you take on by using it.

---

## The Ethical Notice

PHALUS raises serious ethical and legal questions about open source sustainability. It exists to make the machinery of AI-assisted clean room reimplementation visible, auditable, and discussable — not to encourage evasion of license obligations.

You are responsible for understanding the legal implications in your jurisdiction. The legality of AI-assisted clean room reimplementation is unsettled law. For commercial use or high-risk packages, consult a lawyer.

---

## Legal Precedent

The clean room methodology has two foundational pillars in U.S. law:

**Baker v. Selden (1879)**
The Supreme Court held that copyright protects *expression*, not *ideas*. A book describing an accounting system does not prevent others from independently implementing that accounting system. The method or art described in a work is free for all to use; only the specific expression of that description is protected. This principle underlies every clean room reimplementation: reading documentation and independently implementing what it describes is legally distinct from copying code.

**Phoenix Technologies (1984)**
Phoenix Technologies cloned the IBM PC BIOS using a formal clean room process. Two groups of engineers were employed: one group read the IBM BIOS source code and produced a written specification describing what the BIOS did, without reference to how it did it. A second group, who had never seen the IBM source code, implemented a BIOS from that specification only. The result was ruled a legitimate independent creation. This is the direct ancestor of the PHALUS pipeline architecture.

**Google LLC v. Oracle America (2021)**
The Supreme Court found that Google's reimplementation of the Java API in Android constituted fair use. While this decision turned on specific facts and does not establish a blanket rule for API reimplementation, it affirmed that copying an API's structure and function names for the purpose of enabling a compatible implementation can be lawful.

---

## The Open Source Sustainability Problem

PHALUS was built in the context of [Malus](https://malus.sh/), a satirical-but-functional service created by Dylan Ayrey and Mike Nolan and presented at FOSDEM 2026 in the Legal & Policy track. Their talk, "Let's end open source together with this one simple trick," highlighted a real and growing concern.

Open source licenses derive their enforcement power from the cost and difficulty of compliance. A GPL or AGPL license is meaningful partly because incorporating GPL code and then producing a non-GPL alternative that passes legal scrutiny is expensive — it requires legal review, months of engineering work, and substantial documentation. When that cost collapses from months to seconds of LLM compute time, the enforcement mechanism changes character.

Whether that change is net-positive (licenses become suggestions enforced only by social norms) or net-negative (open source contributors lose the legal protections that justified sharing their work) is a genuine and contested question. PHALUS does not take a position. It makes the mechanism visible.

---

## The Training Data Problem

The strongest argument against AI-assisted clean room reimplementation is not the pipeline structure — it is what happened before inference. If the LLM was trained on the original package's source code, its outputs may reproduce implementation patterns, variable names, or algorithmic choices from that training data, regardless of whether the agent was shown the source during the run.

This was directly raised in the Hacker News discussion of Malus: "the contamination happens at the training phase, not the inference phase." PHALUS's similarity scoring is designed to detect this in the output, but cannot eliminate it at the source. The clean room claim is stronger when:

- The similarity score is well below the threshold
- The target language differs from the original (structural divergence)
- The LLM used was not specifically trained on the target codebase
- A human engineer reviews the output before use

For packages where the risk matters, all four mitigations should be applied.

---

## Clean Room Methodology: A Brief History

The concept of clean room reverse engineering predates software. In hardware design, a "clean room" refers to a design process where the team that produces the new design has no access to the original design's internals — only to its observable behaviour and public documentation.

In software, the Phoenix Technologies BIOS project (1984) established the model that PHALUS follows: strict separation between those who read the original and those who write the new implementation. The key legal question in each case is whether the separation was genuine and documented.

What PHALUS adds to this model is:

1. **Automation** — the two-phase process runs in seconds, not months.
2. **Audit trail** — every input, output, and boundary crossing is logged with cryptographic checksums.
3. **Repeatability** — the same pipeline can be re-run on any package with a public API.
4. **Transparency** — the tool is open source; anyone can inspect the pipeline that produces the audit trail.

---

## Your Responsibilities

By using PHALUS you accept that:

- You are responsible for understanding whether the use you intend is lawful in your jurisdiction.
- The audit trail PHALUS produces is evidence of process, not a legal determination of copyright status.
- PHALUS does not assess whether a package's copyright is valid, whether the original license is enforceable, or whether your use constitutes infringement.
- For commercial or high-stakes use, consult a qualified intellectual property attorney before relying on PHALUS output.
- Contributing to open source projects — and supporting the maintainers whose work you depend on — remains the most straightforward way to address open source sustainability.

---

## References

- [Malus — Clean Room as a Service](https://malus.sh/)
- [Malus blog — "Thank You for Your Service"](https://malus.sh/blog.html)
- [FOSDEM 2026 — "Let's end open source together with this one simple trick"](https://fosdem.org/2026/schedule/event/SUVS7G-lets_end_open_source_together_with_this_one_simple_trick/) — Dylan Ayrey & Mike Nolan
- *Baker v. Selden*, 101 U.S. 99 (1879)
- *Google LLC v. Oracle America, Inc.*, 593 U.S. 1 (2021)
- [Phoenix Technologies — Cloning the IBM PC BIOS](https://en.wikipedia.org/wiki/Phoenix_Technologies#Cloning_the_IBM_PC_BIOS)
- [chardet relicensing controversy (March 2026)](https://gigazine.net/gsc_news/en/20260313-malus-open-source/) — real-world AI-assisted relicensing dispute
