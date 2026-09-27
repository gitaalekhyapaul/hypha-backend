import { accessHandlers } from "../../../../../lib/access-requests/runtime.ts";
export const dynamic = "force-dynamic";
export const runtime = "nodejs";
export const GET = accessHandlers.agentList;
export const POST = accessHandlers.agentCreate;
