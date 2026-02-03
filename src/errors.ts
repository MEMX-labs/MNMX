export class MnmxError extends Error {
  constructor(message: string) {
    super(message);
    this.name = 'MnmxError';
  }
}

export class NoRouteFoundError extends MnmxError {
  constructor(
    public readonly fromChain: string,
    public readonly toChain: string,
  ) {
    super(`No viable route found from ${fromChain} to ${toChain}`);
    this.name = 'NoRouteFoundError';
  }
}

export class InsufficientLiquidityError extends MnmxError {
  constructor(
    public readonly bridge: string,
    public readonly required: number,
    public readonly available: number,
  ) {
    super(`Insufficient liquidity on ${bridge}: need ${required}, available ${available}`);
    this.name = 'InsufficientLiquidityError';
  }
}

export class SearchTimeoutError extends MnmxError {
  constructor(
    public readonly elapsedMs: number,
    public readonly timeoutMs: number,
  ) {
    super(`Route search timed out after ${elapsedMs}ms (limit: ${timeoutMs}ms)`);
    this.name = 'SearchTimeoutError';
  }
}

export class BridgeUnavailableError extends MnmxError {
  constructor(public readonly bridge: string) {
    super(`Bridge ${bridge} is currently offline or degraded`);
    this.name = 'BridgeUnavailableError';
  }
}
