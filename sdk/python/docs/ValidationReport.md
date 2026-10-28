# ValidationReport

Result of a validation: the errors, or what the architecture resolves to.

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**errors** | [**List[FieldError]**](FieldError.md) | Every problem found (empty when valid). | 
**estimate** | [**Estimate**](Estimate.md) |  | [optional] 
**hash** | **str** | Hash of the architecture (when valid). | [optional] 
**layout** | [**Layout**](Layout.md) |  | [optional] 
**normalized** | [**Architecture**](Architecture.md) |  | [optional] 
**valid** | **bool** | Whether the document is a valid architecture. | 

## Example

```python
from chess_nnue_lab.models.validation_report import ValidationReport

# TODO update the JSON string below
json = "{}"
# create an instance of ValidationReport from a JSON string
validation_report_instance = ValidationReport.from_json(json)
# print the JSON string representation of the object
print(ValidationReport.to_json())

# convert the object into a dict
validation_report_dict = validation_report_instance.to_dict()
# create an instance of ValidationReport from a dict
validation_report_from_dict = ValidationReport.from_dict(validation_report_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


