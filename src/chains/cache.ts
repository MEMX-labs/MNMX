import type { Chain, ChainConfig } from '../types';

const configCache = new Map<Chain, ChainConfig>();

export function getCachedConfig(chain: Chain): ChainConfig | undefined {
  return configCache.get(chain);
}

export function setCachedConfig(chain: Chain, config: ChainConfig): void {
  configCache.set(chain, config);
}

export function clearConfigCache(): void {
  configCache.clear();
}

export function getConfigCacheSize(): number {
  return configCache.size;
}
