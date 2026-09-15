import assert from "node:assert/strict";

import { AwsClient } from "aws4fetch";

export const boundedJson = async (response, limit = 8 * 1024 * 1024) => {
  const reader = response.body.getReader();
  const chunks = [];
  let size = 0;
  try {
    for (;;) {
      const { done, value } = await reader.read();
      if (done) {
        break;
      }
      size += value.byteLength;
      assert.ok(size <= limit, "operator response exceeds limit");
      chunks.push(value);
    }
  } finally {
    try {
      await reader.cancel();
    } finally {
      reader.releaseLock();
    }
  }
  return JSON.parse(Buffer.concat(chunks).toString("utf-8"));
};

// Node operator only. No public Worker imports this credential-bearing adapter.
export const cloudflareStorage = ({
  accountId,
  databaseId,
  bucketName,
  token,
  accessKeyId,
  secretAccessKey,
  transport = fetch,
}) => {
  assert.match(accountId, /^[a-f0-9]{32}$/u);
  assert.match(databaseId, /^[a-f0-9]{8}-(?:[a-f0-9]{4}-){3}[a-f0-9]{12}$/u);
  assert.match(bucketName, /^[a-z0-9][a-z0-9-]{1,61}[a-z0-9]$/u);
  assert.ok(
    token && accessKeyId && secretAccessKey,
    "explicit D1 and R2 credentials required"
  );
  const signer = new AwsClient({
    accessKeyId,
    region: "auto",
    secretAccessKey,
    service: "s3",
  });
  const query = async (sql, params) => {
    const response = await transport(
      `https://api.cloudflare.com/client/v4/accounts/${accountId}/d1/database/${databaseId}/query`,
      {
        body: JSON.stringify({ params, sql }),
        headers: {
          authorization: `Bearer ${token}`,
          "content-type": "application/json",
        },
        method: "POST",
        redirect: "error",
        signal: AbortSignal.timeout(30000),
      }
    );
    if (!response.ok) {
      await response.body?.cancel();
      throw new Error(
        `D1 request failed (${response.status}); reconcile before retrying`
      );
    }
    const result = await boundedJson(response);
    assert.ok(
      result.success && result.result?.length === 1 && result.result[0].success,
      "D1 operation unconfirmed; reconcile before retrying"
    );
    return result.result[0];
  };
  const request = async (key, method, body) => {
    const url = `https://${accountId}.r2.cloudflarestorage.com/${bucketName}/${key.split("/").map(encodeURIComponent).join("/")}`;
    // sign() performs no I/O; unlike SDK fetch retry policies, exactly one
    // transport attempt is made even on 429/5xx or a lost response.
    const signed = await signer.sign(url, {
      body,
      headers: method === "PUT" ? { "if-none-match": "*" } : {},
      method,
    });
    return transport(signed, {
      redirect: "error",
      signal: AbortSignal.timeout(30000),
    });
  };
  return {
    bucket: {
      async get(key) {
        const response = await request(key, "GET");
        if (response.status === 404) {
          await response.body?.cancel();
          return null;
        }
        if (!response.ok) {
          await response.body?.cancel();
          throw new Error(`R2 read failed (${response.status})`);
        }
        return {
          body: response.body,
          size: Number(response.headers.get("content-length")),
        };
      },
      async put(key, value, options) {
        assert.equal(
          options?.onlyIf?.etagDoesNotMatch,
          "*",
          "create-only write required"
        );
        const response = await request(key, "PUT", value);
        await response.body?.cancel();
        if (response.status === 412) {
          return null;
        }
        if (!response.ok) {
          throw new Error(`R2 write unconfirmed (${response.status})`);
        }
        return { key };
      },
    },
    database: {
      prepare(sql) {
        return {
          bind(...params) {
            return {
              async first() {
                const result = await query(sql, params);
                assert.ok(
                  Array.isArray(result.results),
                  "D1 read returned no row set"
                );
                return result.results[0] ?? null;
              },
              run() {
                return query(sql, params);
              },
            };
          },
        };
      },
    },
  };
};
