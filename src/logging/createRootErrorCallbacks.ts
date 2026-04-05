import type { RootOptions } from "react-dom/client";
import { FRONTEND_LOG_EVENT_IDS } from "./logEvents";
import { createLogger } from "./logger";

type RootErrorCallbacks = Pick<
  RootOptions,
  "onCaughtError" | "onRecoverableError" | "onUncaughtError"
>;

const logger = createLogger("error_boundary");

function stringifyErrorLikeMessage(error: object): string | undefined {
  const message: unknown = Reflect.get(error, "message");
  if (message === undefined || message === null) {
    return undefined;
  }

  if (
    typeof message === "bigint" ||
    typeof message === "boolean" ||
    typeof message === "number" ||
    typeof message === "string" ||
    typeof message === "symbol"
  ) {
    return String(message);
  }

  return undefined;
}

function stringifyUnknownObject(error: object): string {
  try {
    return JSON.stringify(error);
  } catch {
    return Object.prototype.toString.call(error);
  }
}

function isPrimitiveFailureValue(
  error: unknown,
): error is bigint | boolean | number | string | symbol | null {
  if (
    error === null ||
    typeof error === "bigint" ||
    typeof error === "boolean" ||
    typeof error === "number" ||
    typeof error === "string" ||
    typeof error === "symbol"
  ) {
    return true;
  }

  return false;
}

function stringifyPrimitiveFailureValue(
  error: bigint | boolean | number | string | symbol | null,
): string {
  return error === null ? "null" : String(error);
}

function isCallableFailureValue(
  error: unknown,
): error is (...args: never[]) => unknown {
  return typeof error === "function";
}

function stringifyFunctionFailureValue(error: () => unknown): string {
  return error.name === ""
    ? "<anonymous function>"
    : `[function ${error.name}]`;
}

function getFailureMessageFromObject(error: object): string {
  return stringifyErrorLikeMessage(error) ?? stringifyUnknownObject(error);
}

export function getFailureMessage(error: unknown): string {
  if (error instanceof Error) {
    return error.message;
  }

  if (isPrimitiveFailureValue(error)) {
    return stringifyPrimitiveFailureValue(error);
  }

  if (isCallableFailureValue(error)) {
    return stringifyFunctionFailureValue(error);
  }

  if (typeof error === "object") {
    return getFailureMessageFromObject(error);
  }

  return "unknown error";
}

function createComponentStackFields(componentStack: string | undefined): {
  component_stack?: string;
} {
  return componentStack === undefined
    ? {}
    : { component_stack: componentStack };
}

export function createRootErrorCallbacks(): RootErrorCallbacks {
  return {
    onCaughtError: (error, errorInfo): void => {
      logger.error(
        FRONTEND_LOG_EVENT_IDS.errorBoundaryRootRenderErrorCaught,
        error,
        {
          failure_message: getFailureMessage(error),
          ...createComponentStackFields(errorInfo.componentStack),
        },
      );
    },
    onRecoverableError: (error, errorInfo): void => {
      logger.warn(FRONTEND_LOG_EVENT_IDS.errorBoundaryRecoverableRenderError, {
        failure_message: getFailureMessage(error),
        ...createComponentStackFields(errorInfo.componentStack),
      });
    },
    onUncaughtError: (error, errorInfo): void => {
      logger.error(
        FRONTEND_LOG_EVENT_IDS.errorBoundaryUncaughtRenderError,
        error,
        {
          failure_message: getFailureMessage(error),
          ...createComponentStackFields(errorInfo.componentStack),
        },
      );
    },
  };
}

export function resolveCreateRootOptions(
  isProduction: boolean,
): RootErrorCallbacks | undefined {
  return isProduction ? createRootErrorCallbacks() : undefined;
}
