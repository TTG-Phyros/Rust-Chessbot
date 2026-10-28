# MetaApi

All URIs are relative to *http://localhost*

| Method | HTTP request | Description |
|------------- | ------------- | -------------|
| [**getOpenApiDocument**](MetaApi.md#getopenapidocument) | **GET** /api/v1/openapi.json | This document |



## getOpenApiDocument

> any getOpenApiDocument()

This document

### Example

```ts
import {
  Configuration,
  MetaApi,
} from 'chess-nnue-lab';
import type { GetOpenApiDocumentRequest } from 'chess-nnue-lab';

async function example() {
  console.log("🚀 Testing chess-nnue-lab SDK...");
  const api = new MetaApi();

  try {
    const data = await api.getOpenApiDocument();
    console.log(data);
  } catch (error) {
    console.error(error);
  }
}

// Run the test
example().catch(console.error);
```

### Parameters

This endpoint does not need any parameter.

### Return type

**any**

### Authorization

No authorization required

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: `application/json`


### HTTP response details
| Status code | Description | Response headers |
|-------------|-------------|------------------|
| **200** | OpenAPI 3.1 document |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

