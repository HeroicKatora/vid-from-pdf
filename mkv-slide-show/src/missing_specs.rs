use ebml_iterable::specs::{ebml_specification, TagDataType};

#[ebml_specification]
#[derive(Clone, PartialEq, Debug)]
pub enum MatroskaSpec {
    #[id(0x55B0)]
    #[data_type(TagDataType::Master)]
    Color,
    #[id(0x55B1)]
    #[data_type(TagDataType::UnsignedInt)]
    MatrixCoefficients,
    #[id(0x55B2)]
    #[data_type(TagDataType::UnsignedInt)]
    BitsPerChannel,
    // TODO: subsampling.
    #[id(0x55BA)]
    #[data_type(TagDataType::UnsignedInt)]
    TransferCharacteristics,
    #[id(0x55BB)]
    #[data_type(TagDataType::UnsignedInt)]
    Primaries,

    #[id(0x4286)]
    #[data_type(TagDataType::UnsignedInt)]
    EbmlVersion,
    #[id(0x42F7)]
    #[data_type(TagDataType::UnsignedInt)]
    EbmlReadVersion,

    #[id(0x4282)]
    #[data_type(TagDataType::Utf8)]
    EbmlDocType,
    #[id(0x4287)]
    #[data_type(TagDataType::UnsignedInt)]
    EbmlDocTypeVersion,
    #[id(0x4285)]
    #[data_type(TagDataType::UnsignedInt)]
    EbmlDocTypeReadVersion,
}
