# FieldError

One problem of a request, located by a JSON pointer.

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**message** | **str** | What is wrong, and usually how to fix it. | 
**path** | **str** | JSON pointer (RFC 6901) to the offending value; &#x60;\&quot;\&quot;&#x60; is the whole body. | 

## Example

```python
from chess_nnue_lab.models.field_error import FieldError

# TODO update the JSON string below
json = "{}"
# create an instance of FieldError from a JSON string
field_error_instance = FieldError.from_json(json)
# print the JSON string representation of the object
print(FieldError.to_json())

# convert the object into a dict
field_error_dict = field_error_instance.to_dict()
# create an instance of FieldError from a dict
field_error_from_dict = FieldError.from_dict(field_error_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


