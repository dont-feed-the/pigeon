--------------------- MODULE CaptureTransactionSymbolic ----------------------
(* Finite domain for Apalache experiments; the TLAPS proof has no such bound. *)
EXTENDS CaptureTransactionInvariant

ConstInit == Values = {1, 2, 3} /\ Off = {}
=============================================================================
