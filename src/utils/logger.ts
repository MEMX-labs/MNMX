// ─────────────────────────────────────────────────────────────
// MNMX Logger
// ─────────────────────────────────────────────────────────────

import { LogLevel } from '../types/index.js';

const LEVEL_LABELS: Record<LogLevel, string> = {
  [LogLevel.Debug]: 'DEBUG',
  [LogLevel.Info]: 'INFO',
  [LogLevel.Warn]: 'WARN',
  [LogLevel.Error]: 'ERROR',
  [LogLevel.Silent]: 'SILENT',
};

const LEVEL_COLORS: Record<LogLevel, string> = {
  [LogLevel.Debug]: '\x1b[36m',
  [LogLevel.Info]: '\x1b[32m',
  [LogLevel.Warn]: '\x1b[33m',
  [LogLevel.Error]: '\x1b[31m',
  [LogLevel.Silent]: '',
};

const RESET = '\x1b[0m';

function parseLogLevel(value: string | undefined): LogLevel {
  if (!value) return LogLevel.Info;
  const lower = value.toLowerCase();
  switch (lower) {
    case 'debug': return LogLevel.Debug;
    case 'info': return LogLevel.Info;
    case 'warn': return LogLevel.Warn;
    case 'error': return LogLevel.Error;
    case 'silent': return LogLevel.Silent;
    default: return LogLevel.Info;
  }
}

function getEnvLogLevel(): LogLevel {
  if (typeof process !== 'undefined' && process.env) {
    return parseLogLevel(process.env.MNMX_LOG_LEVEL);
  }
  return LogLevel.Info;
}

/**
 * Structured logger with level filtering and prefix support.
 */
export class Logger {
  private level: LogLevel;
  private prefix: string;
  private useColors: boolean;

  constructor(prefix: string = 'mnmx', level?: LogLevel) {
    this.prefix = prefix;
    this.level = level ?? getEnvLogLevel();
    this.useColors =
      typeof process !== 'undefined' &&
      typeof process.stdout !== 'undefined' &&
      (process.stdout as { isTTY?: boolean }).isTTY === true;
  }

  setLevel(level: LogLevel): void {
    this.level = level;
  }

  getLevel(): LogLevel {
    return this.level;
  }

  debug(message: string, ...args: unknown[]): void {
    this.log(LogLevel.Debug, message, ...args);
  }

  info(message: string, ...args: unknown[]): void {
    this.log(LogLevel.Info, message, ...args);
  }

  warn(message: string, ...args: unknown[]): void {
    this.log(LogLevel.Warn, message, ...args);
  }

  error(message: string, ...args: unknown[]): void {
    this.log(LogLevel.Error, message, ...args);
  }

  child(subPrefix: string): Logger {
    const child = new Logger(`${this.prefix}:${subPrefix}`, this.level);
    child.useColors = this.useColors;
    return child;
  }

  private log(level: LogLevel, message: string, ...args: unknown[]): void {
    if (level < this.level) return;
    const timestamp = new Date().toISOString();
    const label = LEVEL_LABELS[level];
    let formatted: string;
    if (this.useColors) {
      const color = LEVEL_COLORS[level];
      formatted = `${color}[${timestamp}] [${label}] [${this.prefix}]${RESET} ${message}`;
    } else {
      formatted = `[${timestamp}] [${label}] [${this.prefix}] ${message}`;
    }
    if (level >= LogLevel.Error) {
      console.error(formatted, ...args);
    } else if (level >= LogLevel.Warn) {
      console.warn(formatted, ...args);
    } else {
      console.log(formatted, ...args);
    }
  }
}

/**
 * Create a logger with the given prefix.
 */
export function createLogger(prefix: string, level?: LogLevel): Logger {
  return new Logger(prefix, level);
}

/** Shared default logger instance. */
export const defaultLogger = new Logger('mnmx');
