# SprtStatus

Where a SPRT stands.

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**llr** | **float** | Log-likelihood ratio of H1 against H0. | 
**lower_bound** | **float** | H0 is accepted when the LLR falls to this bound. | 
**upper_bound** | **float** | H1 is accepted when the LLR rises to this bound. | 
**verdict** | [**Verdict**](Verdict.md) | Conclusion so far. | 

## Example

```python
from chess_nnue_lab.models.sprt_status import SprtStatus

# TODO update the JSON string below
json = "{}"
# create an instance of SprtStatus from a JSON string
sprt_status_instance = SprtStatus.from_json(json)
# print the JSON string representation of the object
print(SprtStatus.to_json())

# convert the object into a dict
sprt_status_dict = sprt_status_instance.to_dict()
# create an instance of SprtStatus from a dict
sprt_status_from_dict = SprtStatus.from_dict(sprt_status_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


