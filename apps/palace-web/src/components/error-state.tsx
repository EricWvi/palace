import { ApiError } from "@/lib/api";
import { Button } from "@/components/ui/button";
export function ErrorState({
  error,
  retry,
}: {
  error: Error;
  retry?: () => void;
}) {
  return (
    <div className="error-state" role="alert">
      <p>{error.message}</p>
      {error instanceof ApiError && error.status === 401 ? (
        <a href="/auth/login">前往登录 →</a>
      ) : (
        retry && (
          <Button variant="outline" onClick={retry}>
            重试
          </Button>
        )
      )}
    </div>
  );
}
