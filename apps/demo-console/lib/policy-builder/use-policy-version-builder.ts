"use client";

import { useRef, useState } from "react";
import type {
  PolicyVersionCandidate,
  PreparePolicyVersionRequest,
} from "../policy-version/types.ts";
import { PortalRequestError } from "../portal-client/browser.ts";
import {
  loadPolicyVersion,
  type PolicyVersionPublication,
  preparePolicyVersion,
  publishPolicyVersion,
} from "../portal-client/policy-versions.ts";
import { defaultPolicyBuilderState, type PolicyBuilderState } from "./model.ts";
import {
  buildPolicyVersionRequest,
  isUncertainPolicyVersionFailure,
  policyVersionFailureMessage,
} from "./versioned.ts";

export function usePolicyVersionBuilder(csrfToken: string, onSessionExpired: () => void) {
  const [builder, setBuilder] = useState(defaultPolicyBuilderState);
  const [familyMode, setFamilyMode] = useState<"new" | "existing">("new");
  const [familyId, setFamilyId] = useState("");
  const [attempt, setAttempt] = useState<PreparePolicyVersionRequest | null>(null);
  const [prepared, setPrepared] = useState<PolicyVersionCandidate | null>(null);
  const [published, setPublished] = useState<PolicyVersionPublication | null>(null);
  const [operation, setOperation] = useState<"compile" | "refresh" | "publish" | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [uncertain, setUncertain] = useState(false);
  const inFlight = useRef(false);
  const request = useRef<PreparePolicyVersionRequest | null>(null);
  const compiled = useRef<PolicyVersionCandidate | null>(null);

  function update<K extends keyof PolicyBuilderState>(field: K, value: PolicyBuilderState[K]) {
    if (request.current || inFlight.current) return;
    setBuilder((current) => ({ ...current, [field]: value }));
    setError(null);
    setNotice(null);
  }

  async function run(action: NonNullable<typeof operation>, work: () => Promise<void>) {
    // Ref guard closes the gap before React renders disabled buttons.
    if (inFlight.current) return;
    inFlight.current = true;
    setOperation(action);
    setError(null);
    setNotice(null);
    try {
      await work();
    } catch (cause) {
      if (action !== "refresh") setUncertain(isUncertainPolicyVersionFailure(cause));
      if (cause instanceof PortalRequestError && cause.status === 401) onSessionExpired();
      else setError(policyVersionFailureMessage(cause));
    } finally {
      inFlight.current = false;
      setOperation(null);
    }
  }

  async function compile() {
    if (compiled.current) return;
    await run("compile", async () => {
      // Retain the complete original request across timeouts, errors, and repeated clicks.
      const exact =
        request.current ??
        buildPolicyVersionRequest(
          builder,
          `candidate-${crypto.randomUUID()}`,
          familyMode === "existing" ? familyId : undefined,
        );
      request.current = exact;
      setAttempt(exact);
      const result = await preparePolicyVersion(exact, csrfToken);
      compiled.current = result;
      setPrepared(result);
      setUncertain(false);
      setNotice("Version compiled and ready for publication.");
    });
  }

  async function refresh() {
    if (!request.current) return;
    const id = request.current.draft.candidateId;
    await run("refresh", async () => {
      const result = await loadPolicyVersion(id);
      compiled.current = result;
      setPrepared(result);
      // Candidate refresh cannot confirm an uncertain publication.
      if (!prepared) setUncertain(false);
      setNotice("Candidate status refreshed.");
    });
  }

  async function publish() {
    if (!compiled.current || published) return;
    const candidate = compiled.current;
    await run("publish", async () => {
      setPublished(await publishPolicyVersion(candidate, csrfToken));
      setUncertain(false);
      setNotice("Version published. No snapshot or capability was created.");
    });
  }

  function startDraft(nextFamily?: string) {
    if (inFlight.current || uncertain) return;
    request.current = null;
    compiled.current = null;
    setAttempt(null);
    setPrepared(null);
    setPublished(null);
    setFamilyMode(nextFamily ? "existing" : "new");
    setFamilyId(nextFamily ?? "");
    setError(null);
    setNotice(null);
  }

  return {
    builder,
    update,
    familyMode,
    familyId,
    selectExistingFamily: (value: string) => {
      if (!request.current && !inFlight.current) {
        setFamilyMode("existing");
        setFamilyId(value);
        setError(null);
        setNotice(null);
      }
    },
    selectFamilyMode: (value: "new" | "existing") => {
      if (!request.current && !inFlight.current) {
        setFamilyMode(value);
        setError(null);
      }
    },
    selectFamilyId: (value: string) => {
      if (!request.current && !inFlight.current) {
        setFamilyId(value);
        setError(null);
      }
    },
    attempt,
    prepared,
    published,
    operation,
    error,
    notice,
    uncertain,
    compile,
    refresh,
    publish,
    startDraft,
  };
}
