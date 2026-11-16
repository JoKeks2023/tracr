# T-0020 Review: save speed and frozen figures

Two bugs from the TRACR-18 click-through, both about a reviewed day:

- **GH #32 / TRACR-46:** saving a review revision (r2) hung for 10 s to 2 min with an idle CPU.
- **GH #31 / TRACR-45:** a reviewed day showed frozen totals next to live counts (unclear windows, "Den Tag einteilen").

Scope: make the review save fast without changing what is derived; make every figure on a reviewed day either frozen or labelled as live. No change to the review model (snapshot by value, discrepancy banner, dismiss).
