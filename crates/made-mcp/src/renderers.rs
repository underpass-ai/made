//! Shared JSON renderers for contract shapes served by more than one backend.
//!
//! Backends convert their domain, protobuf, or fixture input into these small
//! views. JSON field names, nullability, counts, and envelopes live here once.

mod audit_record_view;
mod ceremony_event_page_view;
mod ceremony_instance_listing;
mod ceremony_instance_listing_entry;
mod ceremony_instance_search_page;
mod metric_family_view;
mod metric_sample_view;
mod service_metrics_view;
mod statistics_view;

pub(crate) use audit_record_view::AuditRecordView;
pub(crate) use ceremony_event_page_view::CeremonyEventPageView;
pub(crate) use ceremony_instance_listing::CeremonyInstanceListing;
pub(crate) use ceremony_instance_listing_entry::CeremonyInstanceListingEntry;
pub(crate) use ceremony_instance_search_page::CeremonyInstanceSearchPage;
pub(crate) use metric_family_view::MetricFamilyView;
pub(crate) use metric_sample_view::MetricSampleView;
pub(crate) use service_metrics_view::ServiceMetricsView;
pub(crate) use statistics_view::StatisticsView;
