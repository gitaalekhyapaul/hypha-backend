export class AuthorizationRejectedError extends Error {
  public constructor() {
    super("authorization rejected");
    this.name = "AuthorizationRejectedError";
  }
}

export class ProtectedQueryInfrastructureError extends Error {
  public constructor() {
    super("protected query infrastructure is unavailable");
    this.name = "ProtectedQueryInfrastructureError";
  }
}
